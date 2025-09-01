// file: src/utils/crypto.rs
// version: 1.0.0
// guid: w4x5y6z7-a8b9-0123-4567-890123456789

use anyhow::{Context, Result};
use rand::RngCore;
use sha2::{Digest, Sha256, Sha512};
use std::path::Path;

/// Cryptographic utilities for the installation agent
pub struct CryptoUtils;

impl CryptoUtils {
    /// Generate a secure random password
    pub fn generate_password(length: usize, include_symbols: bool) -> String {
        use rand::distributions::{Alphanumeric, DistString};

        if include_symbols {
            const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                     abcdefghijklmnopqrstuvwxyz\
                                     0123456789\
                                     !@#$%^&*()_+-=[]{}|;:,.<>?";

            let mut rng = rand::thread_rng();
            (0..length)
                .map(|_| {
                    let idx = (rng.next_u32() as usize) % CHARSET.len();
                    CHARSET[idx] as char
                })
                .collect()
        } else {
            Alphanumeric.sample_string(&mut rand::thread_rng(), length)
        }
    }

    /// Generate a cryptographically secure random salt
    pub fn generate_salt(length: usize) -> Vec<u8> {
        let mut salt = vec![0u8; length];
        rand::thread_rng().fill_bytes(&mut salt);
        salt
    }

    /// Hash a password using SHA-512 with salt
    pub fn hash_password_sha512(password: &str, salt: &[u8]) -> Result<String> {
        let mut hasher = Sha512::new();
        hasher.update(password.as_bytes());
        hasher.update(salt);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Hash a password using SHA-256 with salt
    pub fn hash_password_sha256(password: &str, salt: &[u8]) -> Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(salt);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Verify a password against a hash
    pub fn verify_password(password: &str, hash: &str, salt: &[u8], algorithm: HashAlgorithm) -> Result<bool> {
        let computed_hash = match algorithm {
            HashAlgorithm::Sha256 => Self::hash_password_sha256(password, salt)?,
            HashAlgorithm::Sha512 => Self::hash_password_sha512(password, salt)?,
        };

        Ok(computed_hash.eq_ignore_ascii_case(hash))
    }

    /// Create a crypt-style password hash (for /etc/shadow compatibility)
    pub fn create_crypt_hash(password: &str, method: CryptMethod) -> Result<String> {
        let salt = Self::generate_salt(16);
        let salt_str = hex::encode(&salt);

        match method {
            CryptMethod::Sha256 => {
                Ok(format!("$5${}${}", salt_str, Self::hash_password_sha256(password, &salt)?))
            }
            CryptMethod::Sha512 => {
                Ok(format!("$6${}${}", salt_str, Self::hash_password_sha512(password, &salt)?))
            }
        }
    }

    /// Generate a secure random hex string
    pub fn generate_hex_string(length: usize) -> String {
        let bytes = Self::generate_salt(length);
        hex::encode(bytes)
    }

    /// Validate SSH public key format
    pub fn validate_ssh_public_key(key: &str) -> Result<bool> {
        let key = key.trim();

        // Check for basic SSH key format: type key [comment]
        let parts: Vec<&str> = key.split_whitespace().collect();
        if parts.len() < 2 {
            return Ok(false);
        }

        // Check key type
        let valid_types = ["ssh-rsa", "ssh-dss", "ssh-ed25519", "ecdsa-sha2-nistp256",
                          "ecdsa-sha2-nistp384", "ecdsa-sha2-nistp521"];

        if !valid_types.contains(&parts[0]) {
            return Ok(false);
        }

        // Check if the key data is valid base64
        match base64::decode(parts[1]) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Extract SSH key fingerprint
    pub fn get_ssh_key_fingerprint(key: &str) -> Result<String> {
        let key = key.trim();
        let parts: Vec<&str> = key.split_whitespace().collect();

        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Invalid SSH key format"));
        }

        let key_data = base64::decode(parts[1])
            .context("Failed to decode SSH key data")?;

        let mut hasher = Sha256::new();
        hasher.update(&key_data);
        let hash = hasher.finalize();

        Ok(format!("SHA256:{}", base64::encode(&hash)))
    }

    /// Generate LUKS key file
    pub async fn generate_luks_key_file(path: &Path, size: usize) -> Result<()> {
        let key_data = Self::generate_salt(size);

        tokio::fs::write(path, &key_data).await
            .with_context(|| format!("Failed to write LUKS key file: {:?}", path))?;

        // Set restrictive permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(path).await?.permissions();
            perms.set_mode(0o600); // Owner read/write only
            tokio::fs::set_permissions(path, perms).await?;
        }

        Ok(())
    }

    /// Derive key from password using PBKDF2
    pub fn derive_key_pbkdf2(password: &str, salt: &[u8], iterations: u32, key_length: usize) -> Result<Vec<u8>> {
        use pbkdf2::pbkdf2_hmac;
        use sha2::Sha256;

        let mut key = vec![0u8; key_length];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
        Ok(key)
    }

    /// Calculate file checksum
    pub async fn calculate_file_checksum(path: &Path, algorithm: HashAlgorithm) -> Result<String> {
        let content = tokio::fs::read(path).await
            .with_context(|| format!("Failed to read file for checksum: {:?}", path))?;

        match algorithm {
            HashAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(&content);
                Ok(format!("{:x}", hasher.finalize()))
            }
            HashAlgorithm::Sha512 => {
                let mut hasher = Sha512::new();
                hasher.update(&content);
                Ok(format!("{:x}", hasher.finalize()))
            }
        }
    }

    /// Verify file integrity
    pub async fn verify_file_integrity(path: &Path, expected_checksum: &str, algorithm: HashAlgorithm) -> Result<bool> {
        let actual_checksum = Self::calculate_file_checksum(path, algorithm).await?;
        Ok(actual_checksum.eq_ignore_ascii_case(expected_checksum))
    }

    /// Generate Tang binding key for network-bound encryption
    pub fn generate_tang_binding_policy(tang_servers: &[&str]) -> Result<String> {
        // Create a simple Clevis policy for Tang servers
        let policy = if tang_servers.len() == 1 {
            format!(r#"{{"url":"{}"}}"#, tang_servers[0])
        } else {
            let servers: Vec<String> = tang_servers.iter()
                .map(|server| format!(r#"{{"url":"{}"}}"#, server))
                .collect();

            if tang_servers.len() == 2 {
                format!(r#"{{"t":1,"pins":[{}]}}"#, servers.join(","))
            } else {
                // For 3+ servers, require majority
                let threshold = (tang_servers.len() + 1) / 2;
                format!(r#"{{"t":{},"pins":[{}]}}"#, threshold, servers.join(","))
            }
        };

        Ok(policy)
    }

    /// Validate Tang server URL format
    pub fn validate_tang_url(url: &str) -> Result<bool> {
        use url::Url;

        match Url::parse(url) {
            Ok(parsed_url) => {
                // Check for HTTP or HTTPS
                if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
                    return Ok(false);
                }

                // Must have a host
                if parsed_url.host().is_none() {
                    return Ok(false);
                }

                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Generate a secure random UUID
    pub fn generate_secure_uuid() -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }

    /// Constant-time comparison for sensitive data
    pub fn secure_compare(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for (byte_a, byte_b) in a.iter().zip(b.iter()) {
            result |= byte_a ^ byte_b;
        }

        result == 0
    }
}

/// Supported hash algorithms
#[derive(Debug, Clone, Copy)]
pub enum HashAlgorithm {
    Sha256,
    Sha512,
}

/// Supported crypt methods
#[derive(Debug, Clone, Copy)]
pub enum CryptMethod {
    Sha256,
    Sha512,
}

/// SSH key information
#[derive(Debug, Clone)]
pub struct SshKeyInfo {
    pub key_type: String,
    pub fingerprint: String,
    pub comment: Option<String>,
    pub bits: Option<u32>,
}

impl SshKeyInfo {
    /// Parse SSH key information
    pub fn parse(key: &str) -> Result<Self> {
        let key = key.trim();
        let parts: Vec<&str> = key.split_whitespace().collect();

        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Invalid SSH key format"));
        }

        let key_type = parts[0].to_string();
        let fingerprint = CryptoUtils::get_ssh_key_fingerprint(key)?;
        let comment = if parts.len() > 2 {
            Some(parts[2..].join(" "))
        } else {
            None
        };

        // Estimate key size based on type and data length
        let bits = Self::estimate_key_bits(&key_type, parts[1]);

        Ok(Self {
            key_type,
            fingerprint,
            comment,
            bits,
        })
    }

    fn estimate_key_bits(key_type: &str, key_data: &str) -> Option<u32> {
        match key_type {
            "ssh-rsa" => {
                // RSA key size can be estimated from base64 data length
                let data_len = key_data.len();
                if data_len < 400 {
                    Some(2048)
                } else if data_len < 700 {
                    Some(3072)
                } else {
                    Some(4096)
                }
            }
            "ssh-ed25519" => Some(256), // Ed25519 is always 256 bits
            "ecdsa-sha2-nistp256" => Some(256),
            "ecdsa-sha2-nistp384" => Some(384),
            "ecdsa-sha2-nistp521" => Some(521),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_password() {
        let password = CryptoUtils::generate_password(12, false);
        assert_eq!(password.len(), 12);
        assert!(password.chars().all(|c| c.is_alphanumeric()));

        let password_with_symbols = CryptoUtils::generate_password(12, true);
        assert_eq!(password_with_symbols.len(), 12);
    }

    #[test]
    fn test_generate_salt() {
        let salt = CryptoUtils::generate_salt(16);
        assert_eq!(salt.len(), 16);

        let salt2 = CryptoUtils::generate_salt(16);
        assert_ne!(salt, salt2); // Should be different
    }

    #[test]
    fn test_hash_password() {
        let password = "test_password";
        let salt = b"test_salt";

        let hash256 = CryptoUtils::hash_password_sha256(password, salt).unwrap();
        let hash512 = CryptoUtils::hash_password_sha512(password, salt).unwrap();

        assert!(!hash256.is_empty());
        assert!(!hash512.is_empty());
        assert_ne!(hash256, hash512);

        // Verify the hashes
        assert!(CryptoUtils::verify_password(password, &hash256, salt, HashAlgorithm::Sha256).unwrap());
        assert!(CryptoUtils::verify_password(password, &hash512, salt, HashAlgorithm::Sha512).unwrap());
    }

    #[test]
    fn test_validate_ssh_public_key() {
        // Valid SSH key examples
        let valid_key = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQC7... user@host";
        let valid_ed25519 = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5... user@host";

        // These should be valid formats (though the key data is truncated)
        assert!(CryptoUtils::validate_ssh_public_key("ssh-rsa AAAAB3NzaC1yc2EAAAADAQAB").unwrap());
        assert!(CryptoUtils::validate_ssh_public_key("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5").unwrap());

        // Invalid formats
        assert!(!CryptoUtils::validate_ssh_public_key("invalid-key").unwrap());
        assert!(!CryptoUtils::validate_ssh_public_key("ssh-rsa").unwrap());
        assert!(!CryptoUtils::validate_ssh_public_key("").unwrap());
    }

    #[test]
    fn test_create_crypt_hash() {
        let password = "test_password";

        let hash256 = CryptoUtils::create_crypt_hash(password, CryptMethod::Sha256).unwrap();
        let hash512 = CryptoUtils::create_crypt_hash(password, CryptMethod::Sha512).unwrap();

        assert!(hash256.starts_with("$5$"));
        assert!(hash512.starts_with("$6$"));
    }

    #[test]
    fn test_generate_hex_string() {
        let hex = CryptoUtils::generate_hex_string(16);
        assert_eq!(hex.len(), 32); // 16 bytes = 32 hex characters
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_derive_key_pbkdf2() {
        let password = "test_password";
        let salt = b"test_salt";
        let iterations = 1000;
        let key_length = 32;

        let key = CryptoUtils::derive_key_pbkdf2(password, salt, iterations, key_length).unwrap();
        assert_eq!(key.len(), key_length);

        // Same inputs should produce same key
        let key2 = CryptoUtils::derive_key_pbkdf2(password, salt, iterations, key_length).unwrap();
        assert_eq!(key, key2);
    }

    #[test]
    fn test_validate_tang_url() {
        assert!(CryptoUtils::validate_tang_url("http://example.com").unwrap());
        assert!(CryptoUtils::validate_tang_url("https://tang.example.com:7500").unwrap());
        assert!(CryptoUtils::validate_tang_url("http://192.168.1.100:7500").unwrap());

        assert!(!CryptoUtils::validate_tang_url("ftp://example.com").unwrap());
        assert!(!CryptoUtils::validate_tang_url("not-a-url").unwrap());
        assert!(!CryptoUtils::validate_tang_url("").unwrap());
    }

    #[test]
    fn test_secure_compare() {
        let data1 = b"hello";
        let data2 = b"hello";
        let data3 = b"world";
        let data4 = b"hello123";

        assert!(CryptoUtils::secure_compare(data1, data2));
        assert!(!CryptoUtils::secure_compare(data1, data3));
        assert!(!CryptoUtils::secure_compare(data1, data4));
    }

    #[test]
    fn test_generate_tang_binding_policy() {
        let single_server = CryptoUtils::generate_tang_binding_policy(&["http://tang1.example.com"]).unwrap();
        assert!(single_server.contains("http://tang1.example.com"));

        let dual_servers = CryptoUtils::generate_tang_binding_policy(&[
            "http://tang1.example.com",
            "http://tang2.example.com"
        ]).unwrap();
        assert!(dual_servers.contains("\"t\":1"));

        let triple_servers = CryptoUtils::generate_tang_binding_policy(&[
            "http://tang1.example.com",
            "http://tang2.example.com",
            "http://tang3.example.com"
        ]).unwrap();
        assert!(triple_servers.contains("\"t\":2"));
    }
}
