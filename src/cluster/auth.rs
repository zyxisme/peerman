use tonic::{Request, Status};

/// Validate x-cluster-key metadata against the shared secret.
/// Returns Ok if valid, Err(PermissionDenied) if missing or mismatched.
/// If cluster_key is empty on this node (not configured), allows all.
pub fn check_cluster_key<T>(req: &Request<T>, secret: &str) -> Result<(), Status> {
    if secret.is_empty() {
        return Ok(());
    }
    let key = req
        .metadata()
        .get("x-cluster-key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Status::permission_denied("missing x-cluster-key"))?;
    if key != secret {
        return Err(Status::permission_denied("cluster key mismatch"));
    }
    Ok(())
}
