use tonic::{Request, Status};

/// Validate x-cluster-key metadata against the shared secret.
/// Uses constant-time comparison to prevent timing attacks.
/// Returns Ok if valid, Err(PermissionDenied) if missing or mismatched.
/// If cluster_key is empty on this node (not configured), allows all.
#[allow(clippy::result_large_err)]
pub fn check_cluster_key<T>(req: &Request<T>, secret: &str) -> Result<(), Status> {
    if secret.is_empty() {
        tracing::warn!("Cluster key is empty — all inter-node requests are allowed");
        return Ok(());
    }
    let key = req
        .metadata()
        .get("x-cluster-key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Status::permission_denied("missing x-cluster-key"))?;

    if key.len() != secret.len() || !constant_time_eq(key.as_bytes(), secret.as_bytes()) {
        return Err(Status::permission_denied("cluster key mismatch"));
    }
    Ok(())
}

/// Constant-time byte comparison to prevent timing side-channel attacks.
/// Returns true only if both slices are identical in content and length.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"wrong!"));
        assert!(!constant_time_eq(b"short", b"longer"));
    }
}
