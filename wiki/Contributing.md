# Contributing

Thank you for your interest in contributing to Velocity Installer! This guide covers development setup, coding standards, and the contribution process.

## Development Setup

### Prerequisites

- **Rust 1.75+** — [Install Rust](https://rustup.rs/) via rustup
- **Git** — For version control
- **Windows SDK** — For code signing and Win32 APIs (Windows only)
- **WebView2 Runtime** — For modern UI testing (Windows 11 includes it)

### Clone and Build

```bash
git clone https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer.git
cd V.E.L.O.C.I.T.Y.-Installer
cargo build --release
```

### Run Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p velocity-core --lib

# Run with verbose output
cargo test --workspace -- --nocapture
```

### Check Code

```bash
# Check compilation
cargo check --workspace

# Run Clippy lints
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

---

## Project Structure

Velocity is organized as a **7-crate workspace**:

| Crate | Responsibility |
|-------|---------------|
| `velocity-cli` | CLI entry point |
| `velocity-core` | Core engine (extraction, registry, shortcuts, services, encryption, delta, cloud-fetch) |
| `velocity-config` | Config parser and validator |
| `velocity-ui` | Wizard UI (modern + classic) |
| `velocity-compiler` | Builds standalone `.exe` + MSI |
| `velocity-runtime` | Lightweight runtime embedded in installers |
| `velocity-plugin-api` | WASM plugin trait and SDK |

See [[Architecture]] for detailed crate responsibilities and data flow.

---

## Coding Standards

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting (all warnings treated as errors)
- Prefer `thiserror` for typed errors in core crates
- Prefer `anyhow` for flexible errors in runtime/CLI

### Error Handling

- **Never use `unwrap()` in production code** — Use proper error handling with `?` operator
- **Provide helpful error messages** — Include context in error chains
- **Use `CoreError` in velocity-core** — Custom error enum with `thiserror`
- **Use `anyhow::Result` in velocity-runtime** — Flexible error handling at boundaries

### Documentation

- **Document all public APIs** — Use `///` doc comments
- **Include examples** — Show usage in doc comments
- **Keep docs up-to-date** — Update docs when changing APIs

### Testing

- **Write tests for new features** — Unit tests + integration tests
- **Test error paths** — Don't just test the happy path
- **Test cross-platform code** — Use `#[cfg(target_os = "...")]` for platform-specific tests
- **Run full test suite** — `cargo test --workspace` before submitting

### Cross-Platform Code

- **Gate Windows APIs** — Use `#[cfg(target_os = "windows")]`
- **Provide Unix alternatives** — Every Windows function needs a Unix equivalent
- **Test on multiple platforms** — Linux, macOS, Windows
- **Use pure-Rust crates** — Prefer cross-platform dependencies

---

## Contribution Process

### 1. Fork and Branch

```bash
# Fork on GitHub
# Clone your fork
git clone https://github.com/YOUR_USERNAME/V.E.L.O.C.I.T.Y.-Installer.git
cd V.E.L.O.C.I.T.Y.-Installer

# Create a feature branch
git checkout -b feature/my-feature
```

### 2. Make Changes

- Write code following the coding standards above
- Add tests for new features
- Update documentation as needed
- Ensure `cargo test --workspace` passes
- Ensure `cargo clippy --workspace -- -D warnings` passes

### 3. Commit

```bash
git add .
git commit -m "feat: add my feature"
```

**Commit message format:**
- `feat:` — New feature
- `fix:` — Bug fix
- `docs:` — Documentation changes
- `test:` — Test additions/changes
- `refactor:` — Code refactoring
- `chore:` — Build system, CI, dependencies

### 4. Push and PR

```bash
git push origin feature/my-feature
```

Open a Pull Request on GitHub:
- **Title** — Clear, concise description
- **Description** — What changed and why
- **Tests** — Confirm all tests pass
- **Documentation** — Update docs if needed

### 5. Review

- Maintainers will review your PR
- Address feedback and push updates
- Once approved, maintainers will merge

---

## Security Guidelines

### Reporting Vulnerabilities

If you discover a security vulnerability, **do not open a public issue**. Email:
- **security@unitbuilds.com**
- **PGP Key:** Available on keyserver

### Secure Coding Practices

- **Never hardcode secrets** — Use environment variables or secure vaults
- **Validate all inputs** — Sanitize user input before use
- **Use HTTPS only** — Never use HTTP for downloads or API calls
- **Audit unsafe code** — Document safety invariants for all `unsafe` blocks
- **Zeroize sensitive data** — Clear passwords and keys from memory after use

---

## Testing Guidelines

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        let result = my_function("input");
        assert_eq!(result, "expected");
    }

    #[test]
    fn test_error_case() {
        let result = my_function("invalid");
        assert!(result.is_err());
    }
}
```

### Integration Tests

Place integration tests in `tests/` directory:

```rust
// tests/integration_test.rs
use velocity_core::my_module;

#[test]
fn test_integration() {
    // ...
}
```

### Cross-Platform Tests

```rust
#[cfg(target_os = "windows")]
#[test]
fn test_windows_specific() {
    // Windows-only test
}

#[cfg(not(target_os = "windows"))]
#[test]
fn test_unix_specific() {
    // Unix-only test
}
```

---

## Documentation

### Wiki Pages

The wiki is in the `wiki/` directory. When adding features, update relevant wiki pages.

### Doc Comments

```rust
/// Calculates the compression ratio.
///
/// # Arguments
///
/// * `original_size` - Size of the original data in bytes
/// * `compressed_size` - Size of the compressed data in bytes
///
/// # Returns
///
/// Compression ratio as a percentage (0-100)
///
/// # Example
///
/// ```
/// let ratio = compression_ratio(1000, 500);
/// assert_eq!(ratio, 50.0);
/// ```
pub fn compression_ratio(original_size: u64, compressed_size: u64) -> f64 {
    // ...
}
```

---

## Release Process

### Version Bump

```bash
# Update version in Cargo.toml
cargo set-version 1.0.1

# Update CHANGELOG.md
# Commit and tag
git commit -m "chore: bump version to 1.0.1"
git tag v1.0.1
git push origin main --tags
```

### CI/CD

The CI pipeline automatically:
1. Builds release binaries for Windows, Linux, macOS
2. Signs binaries (if secrets configured)
3. Creates GitHub release
4. Uploads signed artifacts

---

## Code of Conduct

- **Be respectful** — Treat all contributors with dignity
- **Be constructive** — Provide helpful feedback
- **Be inclusive** — Welcome diverse perspectives
- **Be professional** — Maintain a professional environment

---

## Getting Help

- **Wiki** — Check the wiki pages first
- **Issues** — Search existing issues
- **Discussions** — Ask questions in GitHub Discussions
- **Discord** — Join our Discord server (link in README)

---

## Recognition

Contributors are recognized in:
- GitHub contributors graph
- Release notes
- Project documentation

Thank you for contributing to Velocity Installer!
