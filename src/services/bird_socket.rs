use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::AppError;

pub struct BirdResponse {
    pub lines: Vec<String>,
}

pub struct BirdSocket {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
}

impl BirdSocket {
    pub async fn connect(socket_path: &str) -> Result<Self, AppError> {
        let path = Path::new(socket_path);
        if !path.exists() {
            return Err(AppError::Internal(format!(
                "BIRD control socket not found at {socket_path}"
            )));
        }

        let stream = UnixStream::connect(path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect to BIRD socket: {e}")))?;

        let (reader, mut writer) = tokio::io::split(stream);

        // Read welcome banner (code 0001)
        let mut buf_reader = BufReader::new(reader);
        let mut welcome = String::new();
        buf_reader
            .read_line(&mut welcome)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read BIRD welcome: {e}")))?;

        if !welcome.starts_with("0001") {
            return Err(AppError::Internal(format!(
                "Unexpected BIRD welcome: {welcome}"
            )));
        }

        // Send restrict to disable interactive formatting
        writer
            .write_all(b"restrict\n")
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send restrict: {e}")))?;

        let mut socket = Self {
            reader: buf_reader,
            writer,
        };

        // Read the restrict response (status code)
        let _response = socket.read_response().await?;
        Ok(socket)
    }

    pub async fn execute(&mut self, command: &str) -> Result<BirdResponse, AppError> {
        self.writer
            .write_all(command.as_bytes())
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write command: {e}")))?;

        if !command.ends_with('\n') {
            self.writer
                .write_all(b"\n")
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write newline: {e}")))?;
        }

        self.read_response().await
    }

    async fn read_response(&mut self) -> Result<BirdResponse, AppError> {
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();
        let mut append_mode = false;

        loop {
            let mut buf = String::new();
            let n =
                self.reader.read_line(&mut buf).await.map_err(|e| {
                    AppError::Internal(format!("Failed to read BIRD response: {e}"))
                })?;

            if n == 0 {
                break;
            }

            let line = buf.trim_end_matches(['\n', '\r']).to_string();

            if line.len() >= 4 && line[..4].chars().all(|c| c.is_ascii_digit()) {
                let code: u16 = line[..4]
                    .parse()
                    .map_err(|_| AppError::Internal(format!("Invalid status code in: {line}")))?;

                match code / 1000 {
                    0 => break, // Success terminal code (0000-0016)
                    1 | 2 => {
                        // Table entry (1xxx) or continuation (2xxx)
                        if !current_line.is_empty() {
                            lines.push(current_line.clone());
                            current_line.clear();
                        }
                        let data = if line.len() > 5 { &line[5..] } else { "" };
                        current_line = data.to_string();
                        append_mode = false;
                    }
                    8 | 9 => {
                        let msg = line[5..].to_string();
                        return Err(AppError::Internal(format!("BIRD error: {msg}")));
                    }
                    _ => break, // Unknown code — treat as terminal
                }
            } else if let Some(data) = line.strip_prefix(' ') {
                if append_mode {
                    current_line.push_str(data);
                } else {
                    current_line.push('\n');
                    current_line.push_str(data);
                }
            } else if let Some(stripped) = line.strip_prefix('+') {
                current_line.push_str(stripped);
                append_mode = true;
            } else if line.is_empty() {
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                }
            } else {
                current_line.push('\n');
                current_line.push_str(&line);
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        Ok(BirdResponse { lines })
    }
}
