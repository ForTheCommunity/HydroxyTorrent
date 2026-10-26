use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ring::pbkdf2;
use std::num::NonZeroU32;
use thiserror::Error;

const HASH_PREFIX: &str = "$pbkdf2-sha256$";
const DEFAULT_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid hash format")]
    InvalidHashFormat,
    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("Invalid iterations count")]
    InvalidIterations,
}

/// Checks if a string is already in the PBKDF2 hash format.
pub fn is_hashed(password: &str) -> bool {
    password.starts_with(HASH_PREFIX)
}

/// Hashes a plain-text password using PBKDF2-HMAC-SHA256 with a random salt.
pub fn hash_password(plain_password: &str) -> String {
    let mut salt = [0u8; SALT_LEN];
    // Generate cryptographically secure random salt using ring
    let rng = ring::rand::SystemRandom::new();
    ring::rand::SecureRandom::fill(&rng, &mut salt).expect("SystemRandom failed");

    let iterations = NonZeroU32::new(DEFAULT_ITERATIONS).unwrap();
    let mut out_key = [0u8; KEY_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        &salt,
        plain_password.as_bytes(),
        &mut out_key,
    );

    let salt_b64 = BASE64.encode(salt);
    let key_b64 = BASE64.encode(out_key);

    format!("{}i={}${}${}", HASH_PREFIX, DEFAULT_ITERATIONS, salt_b64, key_b64)
}

/// Generates a cryptographically secure 64-character hex session token.
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    let rng = ring::rand::SystemRandom::new();
    let _ = ring::rand::SecureRandom::fill(&rng, &mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Verifies a plain-text password against a stored hashed password string.
pub fn verify_password(plain_password: &str, hashed_str: &str) -> bool {
    if !is_hashed(hashed_str) {
        // Plain text comparison fallback if not hashed
        return plain_password == hashed_str;
    }

    let payload = match hashed_str.strip_prefix(HASH_PREFIX) {
        Some(p) => p,
        None => return false,
    };

    let parts: Vec<&str> = payload.split('$').collect();
    if parts.len() != 3 {
        return false;
    }

    let iterations_part = parts[0];
    let salt_part = parts[1];
    let key_part = parts[2];

    let iterations_str = match iterations_part.strip_prefix("i=") {
        Some(s) => s,
        None => return false,
    };

    let iterations_num: u32 = match iterations_str.parse() {
        Ok(num) if num > 0 => num,
        _ => return false,
    };

    let iterations = match NonZeroU32::new(iterations_num) {
        Some(n) => n,
        None => return false,
    };

    let salt = match BASE64.decode(salt_part) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let expected_key = match BASE64.decode(key_part) {
        Ok(k) => k,
        Err(_) => return false,
    };

    pbkdf2::verify(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        &salt,
        plain_password.as_bytes(),
        &expected_key,
    )
    .is_ok()
}

/// Generates a secure random alphanumeric password of the given length.
pub fn generate_random_password(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Updates username and password under `[remote_config]` while preserving existing TOML comments and structure.
pub fn update_credentials_in_toml_content(
    content: &str,
    username: &str,
    hashed_password: &str,
) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut in_remote_config = false;
    let mut username_updated = false;
    let mut password_updated = false;
    let mut remote_section_end = lines.len();

    for (idx, line) in lines.iter_mut().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_remote_config {
                remote_section_end = idx;
                in_remote_config = false;
            }
            if trimmed == "[remote_config]" {
                in_remote_config = true;
            }
            continue;
        }

        if in_remote_config {
            if trimmed.starts_with("username") {
                if let Some((key, _)) = line.split_once('=') {
                    if key.trim() == "username" {
                        *line = format!("username = \"{}\"", username);
                        username_updated = true;
                    }
                }
            } else if trimmed.starts_with("password") {
                if let Some((key, _)) = line.split_once('=') {
                    if key.trim() == "password" {
                        *line = format!("password = \"{}\"", hashed_password);
                        password_updated = true;
                    }
                }
            }
        }
    }

    if in_remote_config {
        remote_section_end = lines.len();
    }

    // If either key wasn't in [remote_config], insert it at the end of [remote_config]
    if !username_updated {
        lines.insert(remote_section_end, format!("username = \"{}\"", username));
        remote_section_end += 1;
    }
    if !password_updated {
        lines.insert(
            remote_section_end,
            format!("password = \"{}\"", hashed_password),
        );
    }

    let mut result = lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "MySecurePassword123!";
        let hashed = hash_password(password);

        assert!(is_hashed(&hashed));
        assert!(verify_password(password, &hashed));
        assert!(!verify_password("WrongPassword", &hashed));
        assert!(!verify_password("MySecurePassword123", &hashed));
    }

    #[test]
    fn test_generate_random_password() {
        let pwd1 = generate_random_password(16);
        let pwd2 = generate_random_password(16);

        assert_eq!(pwd1.len(), 16);
        assert_eq!(pwd2.len(), 16);
        assert_ne!(pwd1, pwd2);

        let hashed = hash_password(&pwd1);
        assert!(verify_password(&pwd1, &hashed));
    }

    #[test]
    fn test_plain_fallback_verify() {
        // Fallback for unhashed strings
        assert!(verify_password("plain123", "plain123"));
        assert!(!verify_password("plain123", "different"));
    }

    #[test]
    fn test_update_credentials_in_toml_content() {
        let sample_toml = r#"# Sample comment
[network_config]
listen_port = 9999

[remote_config]
# enable/disable RPC
rpc_enabled = true
username = ""
password = "plain_user_password"
# end of file
"#;

        let updated = update_credentials_in_toml_content(
            sample_toml,
            "admin",
            "$pbkdf2-sha256$i=100000$salt$hash",
        );

        assert!(updated.contains(r#"username = "admin""#));
        assert!(updated.contains(r#"password = "$pbkdf2-sha256$i=100000$salt$hash""#));
        assert!(updated.contains("# Sample comment"));
        assert!(updated.contains("# enable/disable RPC"));
        assert!(!updated.contains(r#"password = "plain_user_password""#));
    }
}
