/// DN42 convention helpers for ASN-derived values.

/// Listen port: `2` + last 4 digits of ASN.
/// Example: ASN 4242420365 → port 20365
pub fn listen_port_from_asn(asn: i64) -> u32 {
    (20000 + (asn % 10000)) as u32
}

/// Link-local IPv6: `fe80::` + last 4 digits of ASN (strip leading zeros).
/// Example: ASN 4242420365 → "fe80::365"
/// Example: ASN 4242421000 → "fe80::1000"
pub fn link_local_from_asn(asn: i64) -> String {
    let last4 = asn % 10000;
    format!("fe80::{last4}")
}

/// Sanitize peer name into WG interface name.
/// Example: "Aleksana" → "wg-aleksana"
pub fn sanitize_interface_name(name: &str) -> String {
    let sanitized: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive dashes
    let mut result = String::new();
    let mut prev_dash = false;
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_dash {
                result.push(c);
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    // Trim leading/trailing dashes
    let trimmed = result.trim_matches('-');
    // Prepend "wg-" and truncate
    let full = format!("wg-{trimmed}");
    if full.len() > 15 {
        full[..15].to_string()
    } else {
        full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listen_port_from_asn() {
        assert_eq!(listen_port_from_asn(4242420365), 20365);
        assert_eq!(listen_port_from_asn(4242420000), 20000);
        assert_eq!(listen_port_from_asn(4242429999), 29999);
    }

    #[test]
    fn test_link_local_from_asn() {
        assert_eq!(link_local_from_asn(4242420365), "fe80::365");
        assert_eq!(link_local_from_asn(4242421000), "fe80::1000");
        assert_eq!(link_local_from_asn(4242420001), "fe80::1");
        assert_eq!(link_local_from_asn(4242420000), "fe80::0");
    }

    #[test]
    fn test_sanitize_interface_name() {
        assert_eq!(sanitize_interface_name("Aleksana"), "wg-aleksana");
        assert_eq!(sanitize_interface_name("my peer!"), "wg-my-peer");
        assert_eq!(sanitize_interface_name("a--b"), "wg-a-b");
        assert_eq!(sanitize_interface_name("-dash-"), "wg-dash");
    }
}
