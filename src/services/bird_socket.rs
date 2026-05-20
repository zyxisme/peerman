use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::AppError;

const SOCKET_PATH: &str = "/var/run/bird.ctl";

pub struct BirdResponse {
    pub code: u16,
    pub lines: Vec<String>,
}

pub struct BirdSocket {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
}

impl BirdSocket {
    pub async fn connect() -> Result<Self, AppError> {
        let path = Path::new(SOCKET_PATH);
        if !path.exists() {
            return Err(AppError::Internal(format!(
                "BIRD control socket not found at {SOCKET_PATH}"
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
        let mut final_code: u16 = 0;
        let mut current_line = String::new();
        let mut append_mode = false;

        loop {
            let mut buf = String::new();
            let n = self
                .reader
                .read_line(&mut buf)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read BIRD response: {e}")))?;

            if n == 0 {
                break;
            }

            let line = buf.trim_end_matches(['\n', '\r']).to_string();

            // Parse 4-digit status code at start of line
            if line.len() >= 4 && line[..4].chars().all(|c| c.is_ascii_digit()) {
                let code: u16 = line[..4]
                    .parse()
                    .map_err(|_| AppError::Internal(format!("Invalid status code in: {line}")))?;

                let code_type = code / 1000;

                match code_type {
                    0 => {
                        // Success terminal code (0000-0016)
                        final_code = code;
                        break;
                    }
                    1 | 2 => {
                        // Table entry (1xxx) or continuation (2xxx)
                        // Only commit non-empty accumulated line
                        if !current_line.is_empty() {
                            lines.push(current_line.clone());
                            current_line.clear();
                        }
                        // Strip first 5 chars (code + space) and start new entry
                        let data = if line.len() > 5 { &line[5..] } else { "" };
                        current_line = data.to_string();
                        append_mode = false;
                    }
                    8 | 9 => {
                        // Error or fatal
                        final_code = code;
                        let msg = line[5..].to_string();
                        return Err(AppError::Internal(format!("BIRD error: {msg}")));
                    }
                    _ => {
                        // Unknown code — treat as terminal
                        final_code = code;
                        break;
                    }
                }
            } else if line.starts_with(' ') {
                // Continuation line (prefixed with space)
                let data = &line[1..];
                if append_mode {
                    current_line.push_str(data);
                } else {
                    current_line.push('\n');
                    current_line.push_str(data);
                }
            } else if line.starts_with('+') {
                // Append without newline
                let data = &line[1..];
                current_line.push_str(data);
                append_mode = true;
            } else if line.is_empty() {
                // Empty line — separator
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                }
            } else {
                // Unknown prefix — just append as continuation
                current_line.push('\n');
                current_line.push_str(&line);
            }
        }

        // Push any remaining accumulated line
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        Ok(BirdResponse {
            code: final_code,
            lines,
        })
    }
}
