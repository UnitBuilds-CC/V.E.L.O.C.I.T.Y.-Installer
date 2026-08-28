# Security

Velocity Installer implements comprehensive security measures to protect users and their data. This page covers encryption, vulnerability mitigations, audit reports, and the threat model.

## Encryption

### AES-256-GCM Authenticated Encryption

Velocity uses **AES-256-GCM** for payload encryption with password protection:

| Parameter | Value |
|-----------|-------|
| **Algorithm** | AES-256-GCM (authenticated encryption) |
| **Key size** | 256 bits |
| **Nonce size** | 96 bits (12 bytes) |
| **Tag size** | 128 bits (16 bytes) |
| **AAD** | Magic bytes `VELOAE02` (format integrity) |

### Key Derivation

**PBKDF2-HMAC-SHA256** derives the AES key from a user-supplied password:

| Parameter | Value |
|-----------|-------|
| **Iterations** | 600,000 (OWASP 2023 recommendation) |
| **Salt** | 16 bytes from CSPRNG |
| **Output** | 32 bytes (256-bit key) |
| **PRF** | HMAC-SHA256 |

### Wire Format (v2)

```
┌──────────┬──────────┬──────────┬───────────────┬──────────────────────┐
│  Magic   │   Salt   │  Nonce   │ Key Verifier  │  Ciphertext + Tag    │
│  8 bytes │ 16 bytes │ 12 bytes │   32 bytes    │  variable + 16 bytes │
└──────────┴──────────┴──────────┴───────────────┴──────────────────────┘
   VELOAE02  BCryptRNG  BCryptRNG  SHA256(key||v)    AES-256-GCM output
```

### Cryptographic Dependencies

| Dependency | Version | Purpose | Audit Status |
|------------|---------|---------|--------------|
| `aes-gcm` | 0.10.3 | AES-256-GCM authenticated encryption | Well-audited (RustCrypto) |
| `pbkdf2` | 0.12.2 | Password-based key derivation | Well-audited (RustCrypto) |
| `sha2` | 0.10.9 | SHA-256 hashing | Well-audited (RustCrypto) |
| `hmac` | 0.12.x | HMAC construction | Well-audited (RustCrypto) |
| `BCryptGenRandom` | Windows API | CSPRNG | Microsoft OS-level |
| `getrandom` | 0.2 | Cross-platform CSPRNG | Well-audited |

### CSPRNG (Cryptographically Secure Pseudo-Random Number Generator)

Velocity uses platform-specific CSPRNGs:
- **Windows:** `BCryptGenRandom` (kernel-level, FIPS 140-2 compliant)
- **Linux:** `getrandom` syscall
- **macOS:** `getentropy` syscall

All cryptographic randomness (salt, nonce) uses these secure sources.

---

## Vulnerability Mitigations

### Path Traversal Protection

**Zip-slip attacks** are prevented by validating all archive entries:

```rust
fn validate_relative_path(path: &Path) -> Result<()> {
    // Reject absolute paths
    if path.is_absolute() {
        return Err("Absolute paths not allowed");
    }
    // Reject ../ components
    for component in path.components() {
        if component == Component::ParentDir {
            return Err("Path traversal not allowed");
        }
    }
    // Reject null bytes
    if path.to_str().map_or(false, |s| s.contains('\0')) {
        return Err("Null bytes not allowed");
    }
    Ok(())
}
```

### Install Directory Validation

Prevents installation to sensitive system directories:

```rust
fn validate_install_dir(dir: &Path) -> Result<()> {
    let forbidden = [
        "C:\\Windows",
        "C:\\Windows\\System32",
        "C:\\ProgramData",
        // ... more
    ];
    for path in forbidden {
        if dir.starts_with(path) {
            return Err("Cannot install to system directory");
        }
    }
    Ok(())
}
```

### Shell Injection Protection

URLs and paths are validated before passing to shell commands:

```rust
// Validate URL before passing to cmd /C start
if url.contains('&') || url.contains('|') || url.contains('>') {
    return Err("Invalid characters in URL");
}
```

### Null Byte Rejection

All paths are checked for null bytes to prevent null byte injection:

```rust
if path.to_str().map_or(false, |s| s.contains('\0')) {
    return Err("Null bytes not allowed in paths");
}
```

### File Backup Before Overwrite

Automatic `.velocity_backup` files are created before replacing:

```rust
if dest.exists() {
    let backup = dest.with_extension("velocity_backup");
    fs::copy(&dest, &backup)?;
}
```

### File Integrity Verification

SHA256 hash checking for all downloaded files:

```rust
fn verify_checksum(path: &Path, expected: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if actual != expected {
        return Err("Checksum mismatch");
    }
    Ok(())
}
```

---

## Cloud-Fetch Security

### HTTP Timeouts

Hardened HTTP agent with strict timeouts:

| Timeout | Value |
|---------|-------|
| Connect | 10 seconds |
| Read | 30 seconds |
| Write | 10 seconds |

### Redirect Safety

Maximum 10 redirects with validated redirect targets:

```rust
let agent = AgentBuilder::new()
    .redirects(10)  // Max 10 redirects
    .timeout_connect(Duration::from_secs(10))
    .timeout_read(Duration::from_secs(30))
    .build();
```

### Rate Limit Handling

Parses `X-RateLimit-Remaining` headers from GitHub and Gitea:

```rust
if let Some(remaining) = response.header("X-RateLimit-Remaining") {
    if remaining == "0" {
        // Fall back to cached version info
        return use_cached_version();
    }
}
```

### URL Validation

All URLs are validated before use:

```rust
fn validate_url(url: &str) -> Result<()> {
    let parsed = Url::parse(url)?;
    if parsed.scheme() != "https" {
        return Err("Only HTTPS URLs allowed");
    }
    Ok(())
}
```

### Placeholder Substitution

URL templates are safely substituted with validated values:

```rust
fn substitute_placeholders(template: &str, app: &str, version: &str, arch: &str) -> String {
    template
        .replace("{app}", app)
        .replace("{version}", version)
        .replace("{arch}", arch)
}
```

### Partial Download Cleanup

Incomplete downloads are automatically removed on failure:

```rust
let result = download_file(url, &temp_path);
if result.is_err() {
    fs::remove_file(&temp_path)?;  // Cleanup partial download
}
```

### Atomic File Writes

Version files are written atomically (write to .tmp, then rename):

```rust
let tmp_path = path.with_extension("tmp");
fs::write(&tmp_path, content)?;
fs::rename(&tmp_path, path)?;  // Atomic on most filesystems
```

---

## Unsafe Code Audit

Velocity has **25 unsafe blocks** across 9 files. All have been audited for safety:

| Category | Count | Risk Level | Pattern |
|----------|-------|------------|---------|
| MessageBoxW calls | 11 | Low | UTF-16 encode → null-terminate → PCWSTR |
| COM initialization/usage | 3 | Medium | CoInitialize → CoCreateInstance → Release |
| Win32 Security (SID) | 1 | Medium | AllocateAndInitializeSid → CheckTokenMembership → FreeSid |
| Win32 Shell (ShellExecuteEx) | 1 | Medium | Struct init → call → close handle |
| WinHTTP networking | 2 | Medium | Session/connect/request lifecycle with cleanup |
| Named mutex | 3 | Medium | CreateMutex/OpenMutex → ReleaseMutex → CloseHandle |
| Environment broadcast | 1 | Low | SendMessageTimeoutW with static string |
| Known folder paths | 1 | Medium | SHGetKnownFolderPath → convert → CoTaskMemFree |
| Architecture detection | 1 | Medium | GetProcAddress → transmute → call |
| Win32 Window creation | 1 | High | CreateWindowExW + message loop + raw pointers |

**All unsafe blocks have valid safety invariants.** No critical findings.

See the full audit in `docs/SAFETY_AUDIT.md`.

---

## Threat Model

| Threat | Mitigation |
|--------|------------|
| **Brute-force password attack** | PBKDF2 with 600K iterations (2^19.2 cost multiplier) |
| **Salt reuse across encryptions** | 16-byte CSPRNG salt (2^128 possible salts) |
| **Nonce reuse (catastrophic for GCM)** | 12-byte CSPRNG nonce (2^96 possible nonces) |
| **Tampered ciphertext** | AES-GCM authentication tag (128-bit) |
| **Cross-version decryption** | AAD includes format magic bytes |
| **Path traversal (zip-slip)** | All archive entries validated (no `../`, no absolute paths) |
| **Null byte injection** | All paths checked for null bytes |
| **Shell injection** | URLs and paths validated before shell execution |
| **System directory overwrite** | Install directory validated against forbidden paths |
| **Man-in-the-middle** | HTTPS required for all downloads |
| **Rate limit exhaustion** | Auth token support, cached fallback |
| **Partial download corruption** | Checksum verification, partial file cleanup |

---

## Security Recommendations

### For Users

1. **Use strong passwords** — Minimum 8 characters recommended for encrypted payloads
2. **Verify checksums** — Always verify SHA256 checksums of downloaded files
3. **Sign your installers** — Use code signing certificates to establish trust
4. **Keep Velocity updated** — Security patches are released regularly

### For Developers

1. **Never hardcode secrets** — Use environment variables or secure vaults
2. **Validate all inputs** — Sanitize user input before use in shell commands
3. **Use HTTPS only** — Never use HTTP for downloads or API calls
4. **Enable address space layout randomization (ASLR)** — Compile with `opt-level = "z"` and `lto = true`
5. **Audit unsafe code** — Review all `unsafe` blocks for safety invariants

---

## Cryptographic Audit Summary

### Strengths

- **Well-audited primitives** — All crypto from RustCrypto project
- **Strong parameters** — 600K PBKDF2 iterations, 256-bit keys
- **CSPRNG** — Platform-specific secure random number generation
- **Authenticated encryption** — AES-GCM provides confidentiality + integrity
- **Format binding** — AAD prevents cross-version decryption

### Future Improvements

1. **Argon2id** — Consider migrating from PBKDF2 to memory-hard Argon2id (requires format version bump to VELOAE03)
2. **Minimum password length** — Enforce 8-character minimum for encrypted payloads
3. **External review** — Independent cryptographic review recommended before large-scale deployment

---

## Crash Reporting

Velocity includes a panic hook that writes crash backtraces to `%TEMP%/velocity_crashes/` for diagnostics:

```rust
std::panic::set_hook(Box::new(|info| {
    let backtrace = Backtrace::new();
    let crash_log = format!("{:?}\n{:?}", info, backtrace);
    fs::write(crash_path, crash_log).ok();
}));
```

Crash logs are **opt-in** and only written if the user explicitly enables them.

---

## Security Contacts

If you discover a security vulnerability, please report it responsibly:

- **Email:** security@unitbuilds.com
- **PGP Key:** [Available on keyserver]
- **Response time:** Within 48 hours

Please do **not** open a public GitHub issue for security vulnerabilities.

---

## Further Reading

- [[Architecture]] — System design and crate structure
- [[Cloud-Fetch-Installers]] — Cloud-fetch security features
- [[Contributing]] — Security guidelines for contributors
