# sign-release.ps1 — Code-sign Velocity installer executables
#
# Usage:
#   .\scripts\sign-release.ps1 -Path target\release\velocity.exe
#   .\scripts\sign-release.ps1 -Path target\release\velocity-runtime.exe -Fingerprint "ABC123..."
#   .\scripts\sign-release.ps1 -Path output\installer.exe -CertFile cert.pfx -Password (Read-Host -AsSecureString)
#
# Environment variables (alternative to parameters):
#   VELOCITY_SIGN_FINGERPRINT  — Certificate fingerprint (store or user)
#   VELOCITY_SIGN_CERT_FILE    — Path to .pfx certificate file
#   VELOCITY_SIGN_CERT_PASSWORD — Password for the .pfx file (plain text, CI only)
#   VELOCITY_SIGN_TIMESTAMP_URL — Timestamp server URL (default: http://timestamp.digicert.com)

param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [string]$Fingerprint = $env:VELOCITY_SIGN_FINGERPRINT,
    [string]$CertFile = $env:VELOCITY_SIGN_CERT_FILE,
    [string]$CertPassword = $env:VELOCITY_SIGN_CERT_PASSWORD,
    [string]$TimestampUrl = $(if ($env:VELOCITY_SIGN_TIMESTAMP_URL) { $env:VELOCITY_SIGN_TIMESTAMP_URL } else { "http://timestamp.digicert.com" }),
    [string]$Description = ""
)

$ErrorActionPreference = "Stop"

# --- Locate signtool.exe ---
function Find-SignTool {
    # Try Windows SDK paths (most common)
    $sdkRoots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "$env:ProgramFiles\Windows Kits\10\bin"
    )
    foreach ($root in $sdkRoots) {
        if (Test-Path $root) {
            $found = Get-ChildItem -Path $root -Filter "signtool.exe" -Recurse -ErrorAction SilentlyContinue |
                Sort-Object { [version]($_.Directory.Name) } -Descending |
                Select-Object -First 1
            if ($found) { return $found.FullName }
        }
    }

    # Try PATH
    $inPath = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($inPath) { return $inPath.Source }

    return $null
}

$signtool = Find-SignTool
if (-not $signtool) {
    Write-Error "signtool.exe not found. Install the Windows SDK or add it to PATH."
    Write-Host "Download: https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/"
    exit 1
}

Write-Host "Using signtool: $signtool"

# --- Validate input file ---
if (-not (Test-Path $Path)) {
    Write-Error "File not found: $Path"
    exit 1
}

$fullPath = (Resolve-Path $Path).Path
Write-Host "Signing: $fullPath"

# --- Build signtool arguments ---
$args = @("sign", "/v")

if ($Description) {
    $args += @("/d", $Description)
}

# Timestamp (always — unsigned installers without timestamp will fail validation after cert expiry)
$args += @("/tr", $TimestampUrl, "/td", "sha256")

# Certificate selection
if ($Fingerprint) {
    Write-Host "Using certificate fingerprint: $Fingerprint"
    # Try machine store first, then user store
    $args += @("/sha1", $Fingerprint)
}
elseif ($CertFile) {
    if (-not (Test-Path $CertFile)) {
        Write-Error "Certificate file not found: $CertFile"
        exit 1
    }
    Write-Host "Using certificate file: $CertFile"
    $args += @("/f", $CertFile)
    if ($CertPassword) {
        $args += @("/p", $CertPassword)
    }
}
else {
    Write-Error "No certificate specified. Use -Fingerprint, -CertFile, or set VELOCITY_SIGN_FINGERPRINT / VELOCITY_SIGN_CERT_FILE."
    exit 1
}

# SHA256 signing
$args += @("/fd", "sha256")

# The file to sign
$args += $fullPath

# --- Execute ---
Write-Host "Running: $signtool $($args -join ' ')"
& $signtool @args
$exitCode = $LASTEXITCODE

if ($exitCode -ne 0) {
    Write-Error "signtool.exe failed with exit code $exitCode"
    exit $exitCode
}

# --- Verify ---
Write-Host "Verifying signature..."
& $signtool verify /pa /v $fullPath
if ($LASTEXITCODE -ne 0) {
    Write-Warning "Signature verification returned non-zero exit code"
}

Write-Host "Successfully signed: $fullPath" -ForegroundColor Green
