#!/usr/bin/env pwsh
# Velocity Installer vs Inno Setup 7 — Benchmark Comparison

param([switch]$SkipInno)

$ErrorActionPreference = "Stop"
$sampleDir = $PSScriptRoot
$projectRoot = Split-Path (Split-Path $sampleDir)
$velocityExe = Join-Path $projectRoot "target\debug\velocity.exe"
$isccExe = "C:\Users\visse\AppData\Local\Programs\Inno Setup 7\ISCC.exe"
$outputDir = Join-Path $sampleDir "benchmark-output"

New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

function FKB($b) { return [math]::Round($b / 1024, 1) }
function FMB($b) { return [math]::Round($b / 1048576, 2) }
function FSZ($b) { return "$(FKB $b) KB ($(FMB $b) MB)" }

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Velocity vs Inno Setup 7 — Benchmark" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# --- Step 0: Measure raw source files ---
Write-Host "[0/4] Measuring raw source files..." -ForegroundColor Yellow
$files = Get-ChildItem -Path (Join-Path $sampleDir "files") -Recurse -File
$rawSize = ($files | Measure-Object -Property Length -Sum).Sum
$nFiles = $files.Count
Write-Host ("  Files: " + $nFiles)
Write-Host ("  Raw size: " + (FSZ $rawSize))
Write-Host ""

# --- Step 1: Build Velocity installer ---
Write-Host "[1/4] Building Velocity installer..." -ForegroundColor Yellow

if (-not (Test-Path $velocityExe)) {
    Write-Host "  Building release binary..." -ForegroundColor DarkGray
    Push-Location $projectRoot; cargo build --release --workspace 2>&1 | Out-Null; Pop-Location
}

$vOut = Join-Path $outputDir "sample-app-velocity.exe"
$vLog = Join-Path $outputDir "velocity-build-log.txt"

$vTime = Measure-Command {
    Push-Location $sampleDir
    & $velocityExe build -o $vOut 2>&1 | Tee-Object -FilePath $vLog
    Pop-Location
}
$vSize = if (Test-Path $vOut) { (Get-Item $vOut).Length } else { 0 }
$vSec = [math]::Round($vTime.TotalSeconds, 2)

Write-Host ("  Build time: " + $vSec + "s") -ForegroundColor Green
Write-Host ("  Output size: " + (FSZ $vSize))
Write-Host ""

# --- Step 2: Build Inno Setup installer ---
$iSize = 0; $iSec = 0
$iAvail = Test-Path $isccExe

if (-not $SkipInno -and $iAvail) {
    Write-Host "[2/4] Building Inno Setup 7 installer..." -ForegroundColor Yellow
    $iOut = Join-Path $outputDir "sample-app-inno-setup.exe"
    $iLog = Join-Path $outputDir "inno-build-log.txt"

    $iTime = Measure-Command {
        & $isccExe (Join-Path $sampleDir "innosetup-sample-app.iss") 2>&1 | Tee-Object -FilePath $iLog
    }
    $iSize = if (Test-Path $iOut) { (Get-Item $iOut).Length } else { 0 }
    $iSec = [math]::Round($iTime.TotalSeconds, 2)

    Write-Host ("  Build time: " + $iSec + "s") -ForegroundColor Green
    Write-Host ("  Output size: " + (FSZ $iSize))
} elseif ($SkipInno) {
    Write-Host "[2/4] Inno Setup skipped" -ForegroundColor DarkGray
} else {
    Write-Host "[2/4] Inno Setup 7 not found" -ForegroundColor Red
}
Write-Host ""

# --- Step 3: Build tool overhead ---
Write-Host "[3/4] Measuring build tool overhead..." -ForegroundColor Yellow
$vBinSize = if (Test-Path $velocityExe) { (Get-Item $velocityExe).Length } else { 0 }
$iToolSize = 0
if ($iAvail) {
    $iToolSize = (Get-ChildItem "C:\Users\visse\AppData\Local\Programs\Inno Setup 7" -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
}
Write-Host ("  Velocity CLI:  " + (FMB $vBinSize) + " MB - single binary, cross-platform")
if ($iAvail) { Write-Host ("  Inno Setup 7:  " + (FMB $iToolSize) + " MB - Windows-only") }
Write-Host ""

# --- Step 4: Report ---
Write-Host "[4/4] Generating comparison report..." -ForegroundColor Yellow
Write-Host ""

$rpt = Join-Path $outputDir "benchmark-report.txt"
$r = @()
$r += "============================================================"
$r += " Velocity Installer vs Inno Setup 7 — Benchmark Report"
$r += " Generated: " + (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
$r += "============================================================"
$r += ""
$r += "--- Source Files ---"
$r += "  File count:    $nFiles"
$r += "  Raw size:      " + (FSZ $rawSize)
$r += ""
$r += "--- Package Size ---"
$r += "  Velocity:      " + (FSZ $vSize)
if ($iSize -gt 0) {
    $ratio = [math]::Round($vSize / $iSize * 100, 1)
    $r += "  Inno Setup 7:  " + (FSZ $iSize)
    $r += "  Ratio:         $ratio% (Velocity / Inno)"
}
$r += ""
$r += "--- Build Time ---"
$r += "  Velocity:      ${vSec}s"
if ($iSec -gt 0) {
    $tRatio = [math]::Round($vSec / $iSec * 100, 1)
    $r += "  Inno Setup 7:  ${iSec}s"
    $r += "  Ratio:         $tRatio% (Velocity / Inno)"
}
$r += ""
$r += "--- Compression Ratio ---"
$vComp = if ($rawSize -gt 0) { [math]::Round($vSize / $rawSize * 100, 1) } else { 0 }
$r += "  Velocity:      $vComp% of raw size"
if ($iSize -gt 0 -and $rawSize -gt 0) {
    $iComp = [math]::Round($iSize / $rawSize * 100, 1)
    $r += "  Inno Setup 7:  $iComp% of raw size"
}
$r += ""
$r += "--- Build Tool Overhead ---"
$r += "  Velocity CLI:  " + (FMB $vBinSize) + " MB - single binary, cross-platform"
if ($iAvail) { $r += "  Inno Setup 7:  " + (FMB $iToolSize) + " MB - Windows-only" }
$r += ""
$r += "--- Feature Comparison ---"
$r += "  Feature                  | Velocity      | Inno Setup 7"
$r += "  -------------------------|---------------|----------------"
$r += "  Cross-platform           | Yes (W/L/M)   | Windows only"
$r += "  Config format            | TOML          | Pascal Script"
$r += "  Plugin system            | WASM          | None"
$r += "  Auto-update              | Built-in      | Manual"
$r += "  Compression              | zstd/lzma2    | lzma/zlib"
$r += "  Encryption               | AES-256-GCM   | Blowfish"
$r += "  Code signing             | Built-in      | External"
$r += "  Localization             | TOML-based    | Built-in"
$r += "  Modern UI                | HTML/CSS      | Native"
$r += "  Classic UI               | Yes           | Yes"
$r += "  Rollback                 | Yes           | Partial"
$r += "  Dependency install       | Built-in      | Manual"
$r += "  File associations        | Yes           | Yes"
$r += "  Registry ops             | Yes           | Yes"
$r += "  Service management       | Yes           | Yes"
$r += "  Environment variables    | Yes           | Yes"
$r += "  Reboot handling          | Yes           | Yes"
$r += "  Condition evaluation     | Yes           | Pascal Script"
$r += "  Open source              | Apache/MIT    | Inno Setup License"
$r += ""

$r | Out-File -FilePath $rpt -Encoding UTF8
$r | ForEach-Object { Write-Host $_ }

Write-Host ""
Write-Host ("Report saved to: " + $rpt) -ForegroundColor Green
Write-Host ("Build outputs in: " + $outputDir) -ForegroundColor Green
