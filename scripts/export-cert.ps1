# export-cert.ps1 — Export a code-signing certificate to PFX
#
# Usage:
#   .\scripts\export-cert.ps1
#   .\scripts\export-cert.ps1 -Fingerprint "ABC123..."
#   .\scripts\export-cert.ps1 -Fingerprint "ABC123..." -OutputPath "my-cert.pfx"
#
# If no fingerprint is given, lists available code-signing certificates and prompts.
# Password is entered interactively (never hardcoded or logged).

param(
    [string]$Fingerprint = "",
    [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"

# --- Find or select certificate ---
$cert = $null

if ($Fingerprint) {
    $cert = Get-ChildItem "Cert:\CurrentUser\My\$Fingerprint" -ErrorAction SilentlyContinue
    if (-not $cert) {
        $cert = Get-ChildItem "Cert:\LocalMachine\My\$Fingerprint" -ErrorAction SilentlyContinue
    }
    if (-not $cert) {
        Write-Error "Certificate with fingerprint '$Fingerprint' not found in current user or local machine store."
        exit 1
    }
}
else {
    # List code-signing certificates
    $allCerts = @(
        Get-ChildItem "Cert:\CurrentUser\My" -ErrorAction SilentlyContinue |
            Where-Object { $_.EnhancedKeyUsageList -match "Code Signing" -or $_.FriendlyName -match "sign" }
    )
    if ($allCerts.Count -eq 0) {
        Write-Host "No code-signing certificates found in current user store."
        Write-Host ""
        Write-Host "To create a self-signed certificate, run:"
        Write-Host '  New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=YourName" -CertStoreLocation "Cert:\CurrentUser\My"'
        Write-Host ""
        Write-Host "Then re-run this script with -Fingerprint <thumbprint>."
        exit 1
    }

    Write-Host "Available code-signing certificates:"
    Write-Host ""
    for ($i = 0; $i -lt $allCerts.Count; $i++) {
        $c = $allCerts[$i]
        Write-Host ("  [{0}] {1}" -f ($i + 1), $c.Subject)
        Write-Host ("      Thumbprint: {0}" -f $c.Thumbprint)
        Write-Host ("      Expires:    {0}" -f $c.NotAfter.ToString("yyyy-MM-dd"))
        Write-Host ""
    }

    $selection = Read-Host "Select certificate number (1-$($allCerts.Count))"
    $idx = [int]$selection - 1
    if ($idx -lt 0 -or $idx -ge $allCerts.Count) {
        Write-Error "Invalid selection."
        exit 1
    }
    $cert = $allCerts[$idx]
}

Write-Host ""
Write-Host "Selected: $($cert.Subject)"
Write-Host "Thumbprint: $($cert.Thumbprint)"
Write-Host "Expires: $($cert.NotAfter.ToString("yyyy-MM-dd"))"
Write-Host ""

# --- Output path ---
if (-not $OutputPath) {
    $OutputPath = Join-Path $PSScriptRoot "codesign-export.pfx"
}

# --- Prompt for password (interactive, never logged) ---
$securePassword = Read-Host -Prompt "Enter password for the PFX file" -AsSecureString
if (-not $securePassword -or $securePassword.Length -eq 0) {
    Write-Error "Password cannot be empty."
    exit 1
}

# --- Export ---
try {
    Export-PfxCertificate -Cert $cert -FilePath $OutputPath -Password $securePassword | Out-Null
    Write-Host ""
    Write-Host "Exported to: $OutputPath" -ForegroundColor Green
    Write-Host ""
    Write-Host "To sign a release build:"
    Write-Host "  .\scripts\sign-release.ps1 -Path target\release\velocity.exe -CertFile `"$OutputPath`""
    Write-Host ""
    Write-Host "Or set environment variables for CI:"
    Write-Host "  `$env:VELOCITY_SIGN_CERT_FILE = `"$OutputPath`""
    Write-Host '  $env:VELOCITY_SIGN_CERT_PASSWORD = "<your-password>"'
}
catch {
    Write-Error "Failed to export certificate: $_"
    exit 1
}
