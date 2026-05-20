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

/// Validate IPv6 link-local address (fe80::/10)
pub fn validate_ipv6_link_local(ip: &str) -> Result<(), AppError> {
    let addr: std::net::Ipv6Addr = ip
        .parse()
        .map_err(|_| AppError::Validation(format!("Invalid IPv6 address: {ip}")))?;
    if !addr.is_unicast_link_local() {
        return Err(AppError::Validation(format!(
            "IPv6 address {ip} is not link-local (fe80::/10)"
        )));
    }
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
    fn test_validate_ipv6_link_local() {
        assert!(validate_ipv6_link_local("fe80::1").is_ok());
        assert!(validate_ipv6_link_local("fd00::1").is_err());
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
}
