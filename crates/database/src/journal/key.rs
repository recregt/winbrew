use crate::core::Hasher;
use crate::models::shared::hash::HashAlgorithm;

pub fn package_journal_key(package_id: &str, version: &str) -> String {
    let mut key = sanitize_package_key_component(package_id);
    key.push('-');
    key.push_str(&version_hash(version));
    key
}

fn sanitize_package_key_component(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());

    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            normalized.push(ch);
        } else {
            normalized.push('_');
        }
    }

    if normalized.is_empty() {
        "package".to_string()
    } else {
        normalized
    }
}

fn version_hash(version: &str) -> String {
    let mut hasher = Hasher::new(HashAlgorithm::Sha256);
    hasher.update(version.trim().as_bytes());

    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(16);
    const HEX: &[u8; 16] = b"0123456789abcdef";

    for &byte in digest.iter().take(8) {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0F) as usize] as char);
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::package_journal_key;

    #[test]
    fn package_journal_key_includes_sanitized_id_and_version_hash() {
        let key = package_journal_key("winget/Contoso.App", "1.2.3");

        assert_eq!(key, "winget_Contoso.App-c47f5b18b8a430e6");
    }
}
