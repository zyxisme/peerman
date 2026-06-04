use crate::error::AppError;

/// Validate ASN is in DN42 private range (4242420000..=4242429999)
pub fn validate_asn(asn: i64) -> Result<(), AppError> {
    if !(4_242_420_000..=4_242_429_999).contains(&asn) {
        return Err(AppError::Validation(format!(
            "ASN {asn} is not in the DN42 private range (4242420000-4242429999)"
        )));
    }
    Ok(())
}

/// Validate a peer name: alphanumeric + hyphens + underscores, no spaces
pub fn validate_peer_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Validation("Peer name cannot be empty".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Validation(
            "Peer name can only contain alphanumeric characters, hyphens, and underscores".into(),
        ));
    }
    Ok(())
}

/// Validate an IPv4 address string
pub fn validate_ipv4(ip: &str) -> Result<(), AppError> {
    ip.parse::<std::net::Ipv4Addr>()
        .map_err(|_| AppError::Validation(format!("Invalid IPv4 address: {ip}")))?;
    Ok(())
}

/// Validate an IPv6 address string
pub fn validate_ipv6(ip: &str) -> Result<(), AppError> {
    ip.parse::<std::net::Ipv6Addr>()
        .map_err(|_| AppError::Validation(format!("Invalid IPv6 address: {ip}")))?;
    Ok(())
}

/// Validate a WireGuard public key (Base64, 44 chars, decodes to 32 bytes)
pub fn validate_wg_public_key(key: &str) -> Result<(), AppError> {
    use base64::Engine;
    if key.len() != 44 {
        return Err(AppError::Validation(
            "WireGuard key must be 44 characters".into(),
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(key)
        .map_err(|_| AppError::Validation("Invalid WireGuard key (not valid Base64)".into()))?;
    if bytes.len() != 32 {
        return Err(AppError::Validation(
            "WireGuard key must decode to 32 bytes".into(),
        ));
    }
    Ok(())
}

/// Validate WireGuard listen port (1-65535)
pub fn validate_port(port: u32, name: &str) -> Result<(), AppError> {
    if port == 0 {
        return Err(AppError::Validation(format!("{name} cannot be 0")));
    }
    if port > 65535 {
        return Err(AppError::Validation(format!(
            "{name} must be between 1 and 65535, got {port}"
        )));
    }
    Ok(())
}

/// Validate BIRD router ID (must be valid IPv4)
pub fn validate_router_id(id: &str) -> Result<(), AppError> {
    if id.is_empty() {
        return Ok(());
    }
    id.parse::<std::net::Ipv4Addr>().map_err(|_| {
        AppError::Validation(format!("Invalid router ID: '{id}'. Must be IPv4 address"))
    })?;
    Ok(())
}

/// Validate DN42 prefix format
pub fn validate_dn42_prefix(prefix: &str, name: &str) -> Result<(), AppError> {
    if prefix.is_empty() {
        return Ok(());
    }
    if !prefix.contains('/') {
        return Err(AppError::Validation(format!(
            "{name} must be in CIDR format (e.g., 172.20.0.0/14), got '{prefix}'"
        )));
    }
    Ok(())
}

/// Validate a hostname or IP address (for traceroute targets, endpoints, etc.)
pub fn validate_host(target: &str) -> Result<(), AppError> {
    if target.is_empty() {
        return Err(AppError::Validation("Host cannot be empty".into()));
    }
    if target.parse::<std::net::Ipv4Addr>().is_ok() || target.parse::<std::net::Ipv6Addr>().is_ok()
    {
        return Ok(());
    }
    if target
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == ':')
    {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "Invalid host: '{target}'. Must be IP address or hostname"
    )))
}

/// Validate a WireGuard interface name (alphanumeric + hyphens + underscores, max 15 chars)
pub fn validate_wg_interface_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Validation(
            "Interface name cannot be empty".into(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Validation(format!(
            "Invalid interface name: '{name}'. Only alphanumeric, hyphens, underscores allowed"
        )));
    }
    if name.len() > 15 {
        return Err(AppError::Validation(
            "Interface name too long (max 15 characters)".into(),
        ));
    }
    Ok(())
}

/// Validate all settings fields.
pub fn validate_settings(s: &crate::models::settings::Settings) -> Result<(), AppError> {
    if s.local_asn != 0 {
        validate_asn(s.local_asn)?;
    }
    if !s.bird_router_id.is_empty() {
        validate_router_id(&s.bird_router_id)?;
    }
    if s.wg_default_listen_port > 0 {
        validate_port(s.wg_default_listen_port as u32, "wg_default_listen_port")?;
    }
    if !s.dn42_ipv4_prefix.is_empty() {
        validate_dn42_prefix(&s.dn42_ipv4_prefix, "dn42_ipv4_prefix")?;
    }
    if !s.dn42_ipv6_prefix.is_empty() {
        validate_dn42_prefix(&s.dn42_ipv6_prefix, "dn42_ipv6_prefix")?;
    }
    if s.roa_rtr_port > 0 {
        validate_port(s.roa_rtr_port as u32, "roa_rtr_port")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_asn_valid() {
        assert!(validate_asn(4_242_420_000).is_ok());
        assert!(validate_asn(4_242_429_999).is_ok());
    }

    #[test]
    fn test_validate_asn_invalid() {
        assert!(validate_asn(0).is_err());
        assert!(validate_asn(4_242_430_000).is_err());
    }

    #[test]
    fn test_validate_peer_name_valid() {
        assert!(validate_peer_name("test-peer").is_ok());
        assert!(validate_peer_name("peer_01").is_ok());
    }

    #[test]
    fn test_validate_peer_name_empty() {
        assert!(validate_peer_name("").is_err());
    }

    #[test]
    fn test_validate_peer_name_invalid_chars() {
        assert!(validate_peer_name("peer name").is_err());
        assert!(validate_peer_name("peer@host").is_err());
    }

    #[test]
    fn test_validate_ipv4_valid() {
        assert!(validate_ipv4("192.168.1.1").is_ok());
        assert!(validate_ipv4("10.0.0.0").is_ok());
    }

    #[test]
    fn test_validate_ipv4_invalid() {
        assert!(validate_ipv4("256.1.1.1").is_err());
        assert!(validate_ipv4("not-an-ip").is_err());
    }

    #[test]
    fn test_validate_ipv6_valid() {
        assert!(validate_ipv6("::1").is_ok());
        assert!(validate_ipv6("fd00::1").is_ok());
    }

    #[test]
    fn test_validate_ipv6_invalid() {
        assert!(validate_ipv6("not-an-ip").is_err());
    }

    #[test]
    fn test_validate_wg_key_valid() {
        let (_, pub_key) = crate::services::wireguard::generate_keypair();
        assert!(validate_wg_public_key(&pub_key).is_ok());
    }

    #[test]
    fn test_validate_wg_key_wrong_length() {
        assert!(validate_wg_public_key("tooshort").is_err());
    }

    #[test]
    fn test_validate_wg_key_invalid_base64() {
        assert!(validate_wg_public_key("!!!!....!!!!....!!!!....!!!!....!!!!....!!!!").is_err());
    }

    #[test]
    fn test_validate_port() {
        assert!(validate_port(42420, "listen_port").is_ok());
        assert!(validate_port(0, "listen_port").is_err());
        assert!(validate_port(70000, "listen_port").is_err());
    }

    #[test]
    fn test_validate_router_id() {
        assert!(validate_router_id("1.2.3.4").is_ok());
        assert!(validate_router_id("").is_ok());
        assert!(validate_router_id("not-an-ip").is_err());
    }

    #[test]
    fn test_validate_dn42_prefix() {
        assert!(validate_dn42_prefix("172.20.0.0/14", "prefix").is_ok());
        assert!(validate_dn42_prefix("", "prefix").is_ok());
        assert!(validate_dn42_prefix("172.20.0.0", "prefix").is_err());
    }

    #[test]
    fn test_validate_host_ip() {
        assert!(validate_host("10.0.0.1").is_ok());
        assert!(validate_host("::1").is_ok());
    }

    #[test]
    fn test_validate_host_hostname() {
        assert!(validate_host("example.com").is_ok());
        assert!(validate_host("node-1.dn42").is_ok());
    }

    #[test]
    fn test_validate_host_shell_injection() {
        assert!(validate_host("10.0.0.1; rm -rf /").is_err());
        assert!(validate_host("$(whoami)").is_err());
    }

    #[test]
    fn test_validate_wg_interface_name() {
        assert!(validate_wg_interface_name("wg0").is_ok());
        assert!(validate_wg_interface_name("wg-cluster").is_ok());
        assert!(validate_wg_interface_name("").is_err());
        assert!(validate_wg_interface_name("wg0; rm -rf /").is_err());
    }
}
