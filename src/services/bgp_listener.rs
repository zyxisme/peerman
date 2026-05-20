use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const BGP_PORT: u16 = 1790;
const BGP_HEADER_LEN: usize = 19;
// Default hold time: 60s, keepalive = hold/3 = 20s
const KEEPALIVE_INTERVAL_SECS: u64 = 20;

#[derive(Debug, Clone)]
pub struct PathChange {
    pub prefix: String,
    pub path_hash: u64,
    pub node_id: String,
}

pub struct BgpListener {
    listener: TcpListener,
    node_id: String,
}

impl BgpListener {
    pub async fn bind(node_id: String) -> Result<Self, std::io::Error> {
        let addr = format!("[::1]:{BGP_PORT}");
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("BGP listener bound to {addr}");
        Ok(Self { listener, node_id })
    }

    pub async fn run(self, tx: mpsc::Sender<PathChange>) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::info!("BGP connection from {addr}");
                    let tx = tx.clone();
                    let node_id = self.node_id.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_session(stream, &node_id, tx).await {
                            tracing::warn!("BGP session ended: {e}");
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("BGP listener accept error: {e}");
                }
            }
        }
    }
}

async fn handle_session(
    mut stream: impl AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
    node_id: &str,
    tx: mpsc::Sender<PathChange>,
) -> Result<(), String> {
    let hold_time: u16 = 60;

    // Send OPEN message
    let open_msg = build_open(hold_time)?;
    stream
        .write_all(&open_msg)
        .await
        .map_err(|e| format!("Failed to send OPEN: {e}"))?;

    // Read OPEN
    let header = read_bgp_header(&mut stream)
        .await
        .map_err(|e| format!("Failed to read OPEN header: {e}"))?;

    if header.msg_type != 1 {
        return Err(format!("Expected OPEN (1), got {}", header.msg_type));
    }

    let mut body = vec![0u8; header.length as usize - BGP_HEADER_LEN];
    if !body.is_empty() {
        stream
            .read_exact(&mut body)
            .await
            .map_err(|e| format!("Failed to read OPEN body: {e}"))?;
    }

    let remote_hold_time = if body.len() >= 4 {
        u16::from_be_bytes([body[2], body[3]])
    } else {
        hold_time
    };
    let negotiated_hold = hold_time.min(remote_hold_time);

    // Send KEEPALIVE
    let ka = build_keepalive();
    stream
        .write_all(&ka)
        .await
        .map_err(|e| format!("Failed to send initial KEEPALIVE: {e}"))?;

    // Spawn keepalive sender
    let (mut read_stream, mut write_stream) = tokio::io::split(stream);

    let ka_interval = if negotiated_hold > 0 {
        (negotiated_hold as u64 / 3).max(1)
    } else {
        KEEPALIVE_INTERVAL_SECS
    };

    let keepalive_task = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(ka_interval)).await;
            if write_stream.write_all(&build_keepalive()).await.is_err() {
                break;
            }
        }
    });

    // Main read loop: parse UPDATE messages
    loop {
        let header = match read_bgp_header(&mut read_stream).await {
            Ok(h) => h,
            Err(_) => break,
        };

        match header.msg_type {
            2 => {
                // UPDATE
                let payload_len = header.length as usize - BGP_HEADER_LEN;
                let mut payload = vec![0u8; payload_len];
                if payload_len > 0 {
                    read_stream
                        .read_exact(&mut payload)
                        .await
                        .map_err(|e| format!("Failed to read UPDATE: {e}"))?;
                }

                let changes = parse_update(&payload);
                for ch in changes {
                    if tx
                        .send(PathChange {
                            prefix: ch.prefix,
                            path_hash: ch.path_hash,
                            node_id: node_id.to_string(),
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
            3 => {
                // NOTIFICATION — session ends
                let reason = if header.length > BGP_HEADER_LEN as u16 + 1 {
                    let mut r = vec![0u8; (header.length - BGP_HEADER_LEN as u16 - 1) as usize];
                    let _ = read_stream.read_exact(&mut r).await;
                    format!("code={header:?} body={r:?}")
                } else {
                    format!("header={header:?}")
                };
                keepalive_task.abort();
                return Err(format!("BGP NOTIFICATION: {reason}"));
            }
            4 => {
                // KEEPALIVE — drain any body bytes
                let drain = header.length as usize - BGP_HEADER_LEN;
                if drain > 0 {
                    let mut buf = vec![0u8; drain];
                    let _ = read_stream.read_exact(&mut buf).await;
                }
            }
            _ => {
                let drain = header.length as usize - BGP_HEADER_LEN;
                if drain > 0 {
                    let mut buf = vec![0u8; drain];
                    let _ = read_stream.read_exact(&mut buf).await;
                }
            }
        }
    }

    keepalive_task.abort();
    Ok(())
}

#[derive(Debug)]
struct BgpHeader {
    msg_type: u8,
    length: u16,
}

async fn read_bgp_header(
    stream: &mut (impl AsyncReadExt + Unpin),
) -> Result<BgpHeader, String> {
    let mut buf = [0u8; BGP_HEADER_LEN];
    stream
        .read_exact(&mut buf)
        .await
        .map_err(|e| format!("read error: {e}"))?;

    // Check marker (all 1s)
    for &b in &buf[..16] {
        if b != 0xff {
            return Err("BGP marker mismatch".to_string());
        }
    }

    let length = u16::from_be_bytes([buf[16], buf[17]]);
    let msg_type = buf[18];

    Ok(BgpHeader { msg_type, length })
}

fn build_open(hold_time: u16) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    body.extend_from_slice(&[4u8]); // BGP version 4

    // My AS: use 65534 (private ASN placeholder)
    body.extend_from_slice(&(65534u16).to_be_bytes());
    // Hold time
    body.extend_from_slice(&hold_time.to_be_bytes());
    // BGP Identifier
    body.extend_from_slice(&[127, 0, 0, 1]);

    // Optional parameters: AddPath capability (code 69)
    // Capability format: 1B code, 1B length, data
    let addpath_cap = [69, 4, 0, 0, 0, 1]; // AddPath, send+receive for IPv4
    let opt_params_len = addpath_cap.len() as u8;
    body.push(opt_params_len);
    body.extend_from_slice(&addpath_cap);

    build_bgp_msg(1, &body)
}

fn build_keepalive() -> Vec<u8> {
    build_bgp_msg(4, &[]).expect("keepalive body is always under 4096 bytes")
}

fn build_bgp_msg(msg_type: u8, body: &[u8]) -> Result<Vec<u8>, String> {
    let total = BGP_HEADER_LEN + body.len();
    if total > 4096 {
        return Err("BGP message too large".to_string());
    }

    let mut msg = vec![0u8; total];
    // Marker: 16 bytes of 0xff
    for b in &mut msg[..16] {
        *b = 0xff;
    }
    // Length
    msg[16..18].copy_from_slice(&(total as u16).to_be_bytes());
    // Type
    msg[18] = msg_type;
    // Body
    msg[BGP_HEADER_LEN..].copy_from_slice(body);

    Ok(msg)
}

#[derive(Debug)]
struct PathChangeInternal {
    prefix: String,
    path_hash: u64,
}

fn parse_update(payload: &[u8]) -> Vec<PathChangeInternal> {
    let mut results = Vec::new();
    let mut pos = 0;

    if pos + 2 > payload.len() {
        return results;
    }

    // Withdrawn Routes Length
    let withdrawn_len = u16::from_be_bytes([payload[pos], payload[pos + 1]]) as usize;
    pos += 2;

    if withdrawn_len > 0 && pos + withdrawn_len <= payload.len() {
        let withdrawn = &payload[pos..pos + withdrawn_len];
        for prefix in parse_nlri(withdrawn) {
            results.push(PathChangeInternal {
                prefix,
                path_hash: 0, // withdrawal = hash 0
            });
        }
        pos += withdrawn_len;
    }

    if pos + 2 > payload.len() {
        return results;
    }

    // Path Attributes Length
    let path_attr_len = u16::from_be_bytes([payload[pos], payload[pos + 1]]) as usize;
    pos += 2;

    let mut path_hash: u64 = 0;
    if path_attr_len > 0 && pos + path_attr_len <= payload.len() {
        // Compute a simple hash of the path attributes for change detection
        let attrs = &payload[pos..pos + path_attr_len];
        path_hash = hash_path_attributes(attrs);
        pos += path_attr_len;
    }

    // NLRI
    if pos < payload.len() {
        let nlri_data = &payload[pos..];
        for prefix in parse_nlri(nlri_data) {
            results.push(PathChangeInternal {
                prefix,
                path_hash,
            });
        }
    }

    results
}

fn hash_path_attributes(attrs: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let _hasher = DefaultHasher::new();
    // Hash each attribute separately to be order-independent
    let mut i = 0;
    let mut hashes: Vec<u64> = Vec::new();

    while i + 3 <= attrs.len() {
        // Attribute: flags(1) + type(1) + length(1 or 2)
        let flags = attrs[i];
        let attr_type = attrs[i + 1];
        let extended = flags & 0x10 != 0;
        i += 2;

        let attr_len = if extended {
            if i + 2 > attrs.len() {
                break;
            }
            let len = u16::from_be_bytes([attrs[i], attrs[i + 1]]) as usize;
            i += 2;
            len
        } else {
            let len = attrs[i] as usize;
            i += 1;
            len
        };

        // Hash the attribute type + value (but skip AS_PATH if we want to track that)
        let end = (i + attr_len).min(attrs.len());
        let mut h = DefaultHasher::new();
        attr_type.hash(&mut h);
        attrs[i..end].hash(&mut h);
        hashes.push(h.finish());

        i = end;
    }

    // Sort to make order-independent, then combine
    hashes.sort_unstable();
    let mut combined = DefaultHasher::new();
    for h in &hashes {
        h.hash(&mut combined);
    }
    combined.finish()
}

/// Parse NLRI prefixes from BGP UPDATE message format.
/// Each prefix: 1B prefix length (in bits), followed by ceil(prefix_len/8) bytes of the prefix.
fn parse_nlri(data: &[u8]) -> Vec<String> {
    let mut prefixes = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let prefix_len = data[pos] as usize;
        pos += 1;

        let addr_bytes = prefix_len.div_ceil(8);
        if pos + addr_bytes > data.len() {
            break;
        }

        let bytes = &data[pos..pos + addr_bytes];
        pos += addr_bytes;

        let prefix_str = if prefix_len <= 32 {
            // IPv4
            let mut octets = [0u8; 4];
            let copy_len = addr_bytes.min(4);
            octets[..copy_len].copy_from_slice(&bytes[..copy_len]);
            format!(
                "{}.{}.{}.{}/{}",
                octets[0], octets[1], octets[2], octets[3], prefix_len
            )
        } else {
            // IPv6
            let mut octets = [0u8; 16];
            let copy_len = addr_bytes.min(16);
            octets[..copy_len].copy_from_slice(&bytes[..copy_len]);

            // Format as IPv6 address
            let mut parts = Vec::new();
            for i in 0..8 {
                parts.push(format!(
                    "{:x}",
                    u16::from_be_bytes([octets[i * 2], octets[i * 2 + 1]])
                ));
            }
            let ipv6 = parts.join(":");
            format!("{ipv6}/{prefix_len}")
        };

        prefixes.push(prefix_str);
    }

    prefixes
}
