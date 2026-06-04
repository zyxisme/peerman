use crate::error::AppError;

/// Allowed BIRD commands (Looking Glass is read-only).
const ALLOWED_PREFIXES: &[&str] = &[
    "show protocols",
    "show status",
    "show route",
    "show interfaces",
    "show neighbors",
    "show symbols",
];

/// Validate a BIRD command against the allowlist.
pub fn validate_bird_command(command: &str) -> Result<&str, AppError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("BIRD command cannot be empty".into()));
    }
    let lower = trimmed.to_lowercase();
    for prefix in ALLOWED_PREFIXES {
        if lower.starts_with(prefix) {
            return Ok(trimmed);
        }
    }
    Err(AppError::Validation(format!(
        "BIRD command not allowed: '{}'. Allowed: {}",
        trimmed,
        ALLOWED_PREFIXES.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_commands() {
        assert!(validate_bird_command("show protocols").is_ok());
        assert!(validate_bird_command("show protocols all").is_ok());
        assert!(validate_bird_command("show status").is_ok());
        assert!(validate_bird_command("show route for 172.20.0.0/14").is_ok());
        assert!(validate_bird_command("show interfaces").is_ok());
        assert!(validate_bird_command("show neighbors").is_ok());
        assert!(validate_bird_command("show symbols").is_ok());
    }

    #[test]
    fn test_blocked_commands() {
        assert!(validate_bird_command("configure").is_err());
        assert!(validate_bird_command("configure soft").is_err());
        assert!(validate_bird_command("debug all").is_err());
        assert!(validate_bird_command("eval $var").is_err());
        assert!(validate_bird_command("").is_err());
    }

    #[test]
    fn test_case_insensitive() {
        assert!(validate_bird_command("SHOW PROTOCOLS").is_ok());
        assert!(validate_bird_command("Show Status").is_ok());
    }
}
