# Cryptographic Dependency Audit

This document catalogs all cryptographic dependencies used by Velocity Installer,
their versions, upstream audit status, and how they are used in the project.

**Last updated:** Phase 9 (CSPRNG upgrade)

## Summary

| Dependency | Version | Purpose | Audit Status |
|---|---|---|---|
| `aes-gcm` | 0.10.3 | AES-256-GCM authenticated encryption | Well-audited (RustCrypto) |
| `pbkdf2` | 0.12.2 | Password-based key derivation | Well-audited (RustCrypto) |
| `sha2` | 0.10.9 | SHA-256 hashing (key derivation, integrity) | Well-audited (RustCrypto) |
| `hmac` | 0.12.x | HMAC construction | Well-audited (RustCrypto) |
| `BCryptGenRandom` | Windows API | Cryptographic random number generation | Microsoft OS-level CSPRNG |

## Detailed Analysis

### AES-256-GCM (`aes-gcm` 0.10.3)

- **Source:** [RustCrypto/aes-gcm](https://github.com/RustCrypto/AEADs)
- **Used in:** `crates/velocity-core/src/encryption.rs`
- **Purpose:** Authenticated encryption of installer payloads with password protection
- **Parameters:**
  - Key size: 256 bits
  - Nonce size: 96 bits (12 bytes)
  - Tag size: 128 bits (16 bytes, implicit in GCM)
  - AAD: Magic bytes `VELOAE02` (8 bytes) for format integrity
- **Security properties:**
  - Confidentiality: AES-256 in counter mode
  - Integrity: GMAC authentication tag
  - Tamper detection: GCM tag verification fails on any modification
- **Audit status:** Part of the RustCrypto project, which undergoes regular review. The AES-GCM implementation uses constant-time operations and is resistant to timing attacks.
- **Known limitations:** GCM nonces must never be reused with the same key. Our CSPRNG-based nonce generation makes reuse astronomically unlikely (2^-96 per pair).

### PBKDF2-HMAC-SHA256 (`pbkdf2` 0.12.2)

- **Source:** [RustCrypto/PBKDF2](https://github.com/RustCrypto/password-hashes)
- **Used in:** `crates/velocity-core/src/encryption.rs` — `derive_key()`
- **Purpose:** Derive a 256-bit AES key from a user-supplied password
- **Parameters:**
  - Iterations: 600,000 (production), 1,000 (test mode via `#[cfg(test)]`)
  - Salt: 16 bytes from BCryptGenRandom
  - Output: 32 bytes (256-bit key)
  - PRF: HMAC-SHA256
- **Compliance:** Meets OWASP 2023 recommendation of 600,000 iterations for SHA-256
- **Audit status:** Part of the RustCrypto password-hashes project. The implementation is constant-time and resistant to timing side-channels.
- **Design note:** The test mode reduces iterations to 1,000 to keep the test suite fast (~0.15s vs ~90s). This only affects test builds; production always uses 600,000.

### SHA-256 (`sha2` 0.10.9)

- **Source:** [RustCrypto/hashes](https://github.com/RustCrypto/hashes)
- **Used in:**
  - `encryption.rs` — Key verifier computation: `SHA256(key || "velocity_verifier")`
  - `security.rs` — File integrity verification
  - `downloader.rs` — SHA256 checksum verification for downloads
  - `checksum.rs` — File checksum computation
- **Purpose:** General-purpose cryptographic hashing
- **Audit status:** Part of the RustCrypto hashes project. Well-audited, constant-time implementation.

### HMAC (`hmac` 0.12.x)

- **Source:** [RustCrypto/MACs](https://github.com/RustCrypto/MACs)
- **Used in:** Indirectly via `pbkdf2` (PBKDF2 uses HMAC internally)
- **Audit status:** Part of the RustCrypto project.

### BCryptGenRandom (Windows API)

- **Source:** Windows `bcryptprimitives.dll` (kernel-mode CSPRNG)
- **Used in:** `crates/velocity-core/src/encryption.rs` — `fill_random()`
- **Purpose:** Generate cryptographically secure random bytes for:
  - PBKDF2 salt (16 bytes per encryption)
  - AES-GCM nonce (12 bytes per encryption)
- **Security properties:**
  - Backed by the Windows kernel random number generator
  - FIPS 140-2 compliant
  - Automatically seeded from hardware entropy sources
- **Fallback:** If BCryptGenRandom fails (extremely rare), falls back to time+PID+SHA256 with a logged error. This fallback is NOT cryptographically secure but ensures the installer doesn't crash.

## Encryption Protocol Design

### Wire Format (v2)

```
┌──────────┬──────────┬──────────┬───────────────┬──────────────────────┐
│  Magic   │   Salt   │  Nonce   │ Key Verifier  │  Ciphertext + Tag    │
│  8 bytes │ 16 bytes │ 12 bytes │   32 bytes    │  variable + 16 bytes │
└──────────┴──────────┴──────────┴───────────────┴──────────────────────┘
   VELOAE02  BCryptRNG  BCryptRNG  SHA256(key||v)    AES-256-GCM output
```

### Key Derivation

```
password + salt → PBKDF2-HMAC-SHA256 (600K rounds) → 32-byte AES key
```

### Password Verification

Rather than storing a password hash, we derive the AES key and compute:
```
verifier = SHA256(aes_key || "velocity_verifier")
```
On decryption, the same derivation is performed and the verifier is compared. This confirms the correct password without a separate hash step. If the verifier matches but the password is wrong (astronomically unlikely given PBKDF2), the GCM tag will still fail.

### Associated Authenticated Data (AAD)

The magic bytes `VELOAE02` are included as AAD in the AES-GCM encryption. This binds the ciphertext to the format version, preventing cross-version decryption attacks.

## Threat Model

| Threat | Mitigation |
|---|---|
| Brute-force password attack | PBKDF2 with 600K iterations (2^19.2 cost multiplier) |
| Salt reuse across encryptions | 16-byte CSPRNG salt (2^128 possible salts) |
| Nonce reuse (catastrophic for GCM) | 12-byte CSPRNG nonce (2^96 possible nonces) |
| Tampered ciphertext | AES-GCM authentication tag (128-bit) |
| Cross-version decryption | AAD includes format magic bytes |
| Weak passwords | No minimum enforced (installer UX concern), but PBKDF2 makes brute-force expensive |
| CSPRNG failure | Logged error + fallback (degraded but functional) |

## Recommendations

1. **No urgent changes needed.** All cryptographic primitives are from the well-audited RustCrypto project and use standard parameters.

2. **Consider Argon2id in the future.** PBKDF2 is acceptable per OWASP 2023, but Argon2id (memory-hard) provides better resistance against GPU/ASIC attacks. This would require a format version bump (VELOAE03).

3. **Consider a minimum password length.** Currently any non-empty password is accepted. A minimum of 8 characters would significantly increase brute-force cost.

4. **External review recommended before large-scale deployment.** While all primitives are well-audited individually, the overall protocol design (key verifier scheme, AAD construction) has not been independently reviewed by a cryptographer.
