use crate::error::AppError;

/// Blocked characters in WG PostUp/PostDown commands.
const BLOCKED_CHARS: &[char] = &[';', '&', '|', '$', '`', '(', ')', '{', '}'];

/// Validate WG PostUp/PostDown commands.
pub fn validate_post_script(script: &str) -> Result<(), AppError> {
    if script.is_empty() {
        return Ok(());
    }
    for ch in BLOCKED_CHARS {
        if script.contains(*ch) {
            return Err(AppError::Validation(format!(
                "PostUp/PostDown command contains blocked character '{ch}': '{script}'"
            )));
        }
    }
    if script.contains(">>") || script.contains("<<") {
        return Err(AppError::Validation(
            "PostUp/PostDown cannot contain redirection operators".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_post_scripts() {
        assert!(validate_post_script("").is_ok());
        assert!(validate_post_script("ip route add 10.0.0.0/8 via 172.20.1.2").is_ok());
        assert!(validate_post_script("iptables -A FORWARD -i wg0 -j ACCEPT").is_ok());
    }

    #[test]
    fn test_blocked_shell_injection() {
        assert!(validate_post_script("ip route add 10.0.0.0/8; rm -rf /").is_err());
        assert!(validate_post_script("ip route && echo pwned").is_err());
        assert!(validate_post_script("cat $(whoami)").is_err());
        assert!(validate_post_script("echo `id`").is_err());
    }

    #[test]
    fn test_blocked_redirection() {
        assert!(validate_post_script("echo test >> /etc/passwd").is_err());
    }
}
