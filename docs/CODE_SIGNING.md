# Code Signing Guide

Velocity Installer supports Authenticode code signing via `signtool.exe`.
Signed installers eliminate Windows SmartScreen warnings and establish trust
with your users.

## Prerequisites

1. **Code signing certificate** — Purchase from a CA (DigiCert, Sectigo, etc.)
   or use a self-signed certificate for testing.
2. **Windows SDK** — Provides `signtool.exe` (included with Visual Studio).
3. **Timestamp server** — Required for signatures to remain valid after
   certificate expiry. Recommended: `http://timestamp.digicert.com`

## Quick Start

```bash
# Build the installer
velocity build

# Sign with a certificate file
velocity sign output/MyApp_Setup.exe \
    --cert my_cert.pfx \
    --timestamp http://timestamp.digicert.com \
    --description "My Application Installer"

# Verify the signature
velocity sign --verify output/MyApp_Setup.exe
```

## Signing Methods

### Certificate File (PFX/P12)

```bash
velocity sign installer.exe --cert certificate.pfx
```

### Certificate Fingerprint

```bash
velocity sign installer.exe --fingerprint "AB:CD:EF:01:23:45:67:89:..."
```

### Certificate Subject Name

```bash
velocity sign installer.exe --subject "My Company Inc"
```

### Default Certificate Store

If no certificate option is specified, `signtool.exe` will attempt to use
the first valid code signing certificate found in the user's certificate store.

## CI/CD Integration

### GitHub Actions

```yaml
- name: Sign installer
  env:
    CERTIFICATE_BASE64: ${{ secrets.CERTIFICATE_BASE64 }}
    CERTIFICATE_PASSWORD: ${{ secrets.CERTIFICATE_PASSWORD }}
  run: |
    # Decode certificate from secret
    echo "$CERTIFICATE_BASE64" | base64 -d > cert.pfx
    # Sign the installer
    velocity sign output/MyApp_Setup.exe \
        --cert cert.pfx \
        --timestamp http://timestamp.digicert.com
    # Clean up
    rm cert.pfx
```

### Azure DevOps

Use the `AzureSignTool` extension or the `sign` command with a service
connection that provides certificate access.

## EV Code Signing Certificates

For production use, we recommend an **EV (Extended Validation) code signing
certificate**:

- **Immediate SmartScreen trust** — No "Windows protected your PC" warning
- **Hardware token required** — Certificate is stored on a USB token (FIPS 140-2)
- **Higher assurance** — Validates your organization's legal identity

When using a hardware token, `signtool.exe` communicates with the token via
the Windows Certificate Store. Use `--fingerprint` to select the correct cert.

## Timestamping

Always timestamp signed installers. Without a timestamp, the signature
becomes invalid when the certificate expires.

```bash
velocity sign installer.exe \
    --cert cert.pfx \
    --timestamp http://timestamp.digicert.com
```

The `--timestamp` URL depends on your CA:
| CA | Timestamp URL |
|---|---|
| DigiCert | `http://timestamp.digicert.com` |
| Sectigo | `http://timestamp.sectigo.com` |
| GlobalSign | `http://timestamp.globalsign.com/scripts/timestamp.dll` |
| Let's Encrypt | N/A (no code signing certs) |

## Troubleshooting

### "signtool.exe not found"

Install the Windows SDK or add `signtool.exe` to your PATH. The Velocity CLI
searches these locations automatically:
- `C:\Program Files (x86)\Windows Kits\10\bin\*\x64\`
- `C:\Program Files\Windows Kits\10\bin\*\x64\`
- `C:\Program Files (x86)\Windows Kits\8.1\bin\`

### "The specified PFX file is invalid"

- Verify the password is correct
- Ensure the certificate includes the private key
- Check the certificate hasn't expired

### SmartScreen still shows warning

- New certificates need to build reputation — sign consistently over time
- Consider upgrading to an EV certificate for immediate trust
- Ensure timestamping was used during signing
