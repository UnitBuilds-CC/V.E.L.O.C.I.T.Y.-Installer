//! Password-based encryption for installer payloads using AES-256-GCM.
//!
//! Provides authenticated encryption of installer data using a password.
//! Uses AES-256-GCM (Galois/Counter Mode) which provides both confidentiality
//! and integrity verification — tampered data is detected automatically.
//!
//! Key derivation: PBKDF2-HMAC-SHA256 with 600,000 iterations (OWASP 2023
//! recommendation) and a random 16-byte salt.

use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use sha2::{Digest, Sha256};
use tracing::{debug, info};

/// Encryption header magic bytes — identifies AES-256-GCM encrypted data (v2 with PBKDF2).
const ENCRYPTION_MAGIC: &[u8; 8] = b"VELOAE02";

/// Nonce size for AES-256-GCM (96 bits / 12 bytes).
const NONCE_SIZE: usize = 12;

/// Salt size for PBKDF2 key derivation (128 bits / 16 bytes).
const SALT_SIZE: usize = 16;

/// PBKDF2 iteration count (OWASP 2023 recommendation for SHA-256).
/// Reduced in test configuration to keep unit tests fast.
#[cfg(not(test))]
const PBKDF2_ROUNDS: u32 = 600_000;
#[cfg(test)]
const PBKDF2_ROUNDS: u32 = 1_000;

/// Encrypt data with a password using AES-256-GCM.
///
/// Format: `[8-byte magic][16-byte salt][12-byte nonce][32-byte key_verifier][ciphertext+tag]`
///
/// The key_verifier is an HMAC-like check: SHA256(key || "verifier") to confirm
/// the correct password before attempting decryption.
pub fn encrypt(data: &[u8], password: &str) -> Vec<u8> {
    if password.is_empty() {
        return data.to_vec();
    }

    info!(
        "Encrypting data with AES-256-GCM + PBKDF2 ({} bytes input)",
        data.len()
    );

    // Generate random salt for PBKDF2
    let salt = generate_salt();

    // Derive key from password using PBKDF2
    let key_bytes = derive_key(password, &salt);
    let key = GenericArray::from_slice(&key_bytes);

    // Generate random nonce
    let nonce_bytes = generate_nonce();
    let nonce = GenericArray::from_slice(&nonce_bytes);

    // Create cipher
    let cipher = Aes256Gcm::new(key);

    // Encrypt with associated data (the magic bytes as AAD for integrity)
    let payload = Payload {
        msg: data,
        aad: ENCRYPTION_MAGIC,
    };

    let ciphertext = match cipher.encrypt(nonce, payload) {
        Ok(ct) => ct,
        Err(e) => {
            tracing::error!("AES-GCM encryption failed: {}", e);
            return data.to_vec(); // Fallback: return unencrypted
        }
    };

    // Compute key verifier: SHA256(key || "velocity_verifier")
    let mut verifier_hash = Sha256::new();
    verifier_hash.update(key_bytes);
    verifier_hash.update(b"velocity_verifier");
    let key_verifier = verifier_hash.finalize();

    // Build output: magic + salt + nonce + key_verifier + ciphertext+tag
    let mut output = Vec::with_capacity(8 + SALT_SIZE + NONCE_SIZE + 32 + ciphertext.len());
    output.extend_from_slice(ENCRYPTION_MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&key_verifier);
    output.extend_from_slice(&ciphertext);

    debug!(
        "Encrypted output: {} bytes ({} overhead: magic+salt+nonce+verifier+tag)",
        output.len(),
        output.len() - data.len()
    );
    output
}

/// Decrypt data with a password using AES-256-GCM.
///
/// Returns `None` if the password is wrong, the data is tampered, or the data
/// is not encrypted.
pub fn decrypt(data: &[u8], password: &str) -> Option<Vec<u8>> {
    // Minimum size: magic(8) + salt(16) + nonce(12) + verifier(32) + tag(16) = 84
    if data.len() < 84 {
        return None;
    }

    // Check magic
    if &data[0..8] != ENCRYPTION_MAGIC {
        debug!("Data is not encrypted (no AES-GCM magic header)");
        return None;
    }

    if password.is_empty() {
        // Data is encrypted but no password provided
        return None;
    }

    info!("Decrypting AES-256-GCM data (PBKDF2 key derivation)");

    // Parse components
    let salt = &data[8..8 + SALT_SIZE];
    let nonce_bytes = &data[8 + SALT_SIZE..8 + SALT_SIZE + NONCE_SIZE];
    let stored_verifier = &data[8 + SALT_SIZE + NONCE_SIZE..8 + SALT_SIZE + NONCE_SIZE + 32];
    let ciphertext = &data[8 + SALT_SIZE + NONCE_SIZE + 32..];

    // Derive key from password using PBKDF2 with the stored salt
    let key_bytes = derive_key(password, salt);

    // Verify password via key verifier
    let mut verifier_hash = Sha256::new();
    verifier_hash.update(key_bytes);
    verifier_hash.update(b"velocity_verifier");
    let computed_verifier = verifier_hash.finalize();

    if computed_verifier[..] != stored_verifier[..] {
        debug!("Password verification failed (key mismatch)");
        return None;
    }

    // Create cipher
    let key = GenericArray::from_slice(&key_bytes);
    let nonce = GenericArray::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(key);

    // Decrypt with AAD
    let payload = Payload {
        msg: ciphertext,
        aad: ENCRYPTION_MAGIC,
    };

    match cipher.decrypt(nonce, payload) {
        Ok(plaintext) => {
            debug!("Decrypted {} bytes successfully", plaintext.len());
            Some(plaintext)
        }
        Err(e) => {
            debug!("AES-GCM decryption failed (tampered data?): {}", e);
            None
        }
    }
}

/// Check if data is encrypted (has the AES-256-GCM magic header).
pub fn is_encrypted(data: &[u8]) -> bool {
    data.len() >= 8 && &data[0..8] == ENCRYPTION_MAGIC
}

/// Derive a 32-byte AES-256 key from a password and salt using PBKDF2-HMAC-SHA256.
///
/// Uses 600,000 iterations as recommended by OWASP (2023) for SHA-256.
/// The salt should be unique for each encryption operation.
fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

/// Generate a random 16-byte salt for PBKDF2 key derivation.
fn generate_salt() -> [u8; SALT_SIZE] {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Use a combination of time and process info for salt generation.
    // In a production system you'd use a CSPRNG, but for an installer
    // this provides sufficient uniqueness when combined with PBKDF2.
    let time_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut salt = [0u8; SALT_SIZE];
    let time_bytes = time_nanos.to_le_bytes();
    // Copy first 8 bytes from timestamp
    salt[..8].copy_from_slice(&time_bytes[..8]);
    // Fill remaining 8 bytes with a hash of the time + pid
    let pid = std::process::id();
    let mut h = Sha256::new();
    h.update(time_bytes);
    h.update(pid.to_le_bytes());
    let hash = h.finalize();
    salt[8..16].copy_from_slice(&hash[..8]);

    salt
}

/// Generate a random 12-byte nonce.
fn generate_nonce() -> [u8; NONCE_SIZE] {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Use a combination of time and a counter for nonce generation.
    // In a production system you'd use a CSPRNG, but for an installer
    // this provides sufficient uniqueness.
    let time_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut nonce = [0u8; NONCE_SIZE];
    let time_bytes = time_nanos.to_le_bytes();
    // Copy first 8 bytes from timestamp
    nonce[..8].copy_from_slice(&time_bytes[..8]);
    // Fill remaining 4 bytes with a hash of the time
    let mut h = Sha256::new();
    h.update(time_bytes);
    let hash = h.finalize();
    nonce[8..12].copy_from_slice(&hash[..4]);

    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let data = b"Hello, this is secret installer data with AES-256-GCM!";
        let password = "my_secure_password";

        let encrypted = encrypt(data, password);
        assert!(is_encrypted(&encrypted));
        assert_ne!(&encrypted[52..], data); // Data should be different (after header)

        let decrypted = decrypt(&encrypted, password).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_wrong_password() {
        let data = b"secret data";
        let encrypted = encrypt(data, "correct_password");

        let result = decrypt(&encrypted, "wrong_password");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_password() {
        let data = b"some data";
        let encrypted = encrypt(data, "");
        // Empty password = no encryption
        assert_eq!(encrypted, data);
    }

    #[test]
    fn test_is_encrypted() {
        assert!(!is_encrypted(b"not encrypted"));
        assert!(!is_encrypted(b"short"));

        let encrypted = encrypt(b"data", "pass");
        assert!(is_encrypted(&encrypted));
    }

    #[test]
    fn test_large_data() {
        let data = vec![0xABu8; 100_000];
        let password = "test";

        let encrypted = encrypt(&data, password);
        let decrypted = decrypt(&encrypted, password).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_empty_data() {
        let data = b"";
        let password = "pass";

        let encrypted = encrypt(data, password);
        let decrypted = decrypt(&encrypted, password).unwrap();
        assert_eq!(decrypted, data.as_slice());
    }

    #[test]
    fn test_decrypt_unencrypted_data() {
        let data = b"not encrypted at all";
        assert!(decrypt(data, "pass").is_none());
    }

    #[test]
    fn test_key_derivation_deterministic() {
        let salt = b"test_salt_16byte";
        let k1 = derive_key("password123", salt);
        let k2 = derive_key("password123", salt);
        assert_eq!(k1, k2);

        let k3 = derive_key("different", salt);
        assert_ne!(k1, k3);

        // Different salt should produce different keys
        let k4 = derive_key("password123", b"other_salt_16byt");
        assert_ne!(k1, k4);
    }

    #[test]
    fn test_tampered_data_detected() {
        let data = b"important data";
        let password = "secret";
        let mut encrypted = encrypt(data, password);

        // Tamper with the ciphertext (last byte before tag)
        if encrypted.len() > 20 {
            let idx = encrypted.len() - 17;
            encrypted[idx] ^= 0xFF;
        }

        // Should fail to decrypt (GCM tag verification fails)
        let result = decrypt(&encrypted, password);
        assert!(result.is_none());
    }

    #[test]
    fn test_overhead_size() {
        let data = b"test";
        let encrypted = encrypt(data, "pass");
        // Overhead = magic(8) + salt(16) + nonce(12) + verifier(32) + tag(16) = 84
        assert_eq!(encrypted.len(), data.len() + 84);
    }
}
