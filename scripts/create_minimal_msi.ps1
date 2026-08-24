# Create a minimal valid MSI with SummaryInfo
$msiPath = "C:\temp\minimal_valid.msi"
if (Test-Path $msiPath) { Remove-Item $msiPath -Force }

$installer = New-Object -ComObject WindowsInstaller.Installer

# Step 1: Create database and add tables
$db = $installer.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $installer, @($msiPath, 3))

# Create Property table
$sql = "CREATE TABLE ``Property`` (``Property`` CHAR(72) NOT NULL LOCALIZABLE, ``Value`` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY ``Property``)"
$v = $db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @($sql))
$v.GetType().InvokeMember("Execute", "InvokeMethod", $null, $v, $null)
$v.GetType().InvokeMember("Close", "InvokeMethod", $null, $v, $null)

# Insert properties
$productCode = "{" + [guid]::NewGuid().ToString().ToUpper() + "}"
$upgradeCode = "{" + [guid]::NewGuid().ToString().ToUpper() + "}"
$props = @(
    @("ProductName", "Velocity Test"),
    @("ProductVersion", "1.0.0"),
    @("Manufacturer", "Velocity Corp"),
    @("ProductCode", $productCode),
    @("UpgradeCode", $upgradeCode),
    @("ProductLanguage", "1033")
)
foreach ($p in $props) {
    $sql = "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('$($p[0])', '$($p[1])')"
    $v = $db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @($sql))
    $v.GetType().InvokeMember("Execute", "InvokeMethod", $null, $v, $null)
    $v.GetType().InvokeMember("Close", "InvokeMethod", $null, $v, $null)
}

# Commit
$db.GetType().InvokeMember("Commit", "InvokeMethod", $null, $db, $null)
Write-Host "Step 1: Database created and committed"

# Step 2: Set SummaryInfo using Installer.SummaryInformation
# The Installer.SummaryInformation takes (database, path, updateCount)
# When database is empty string, it uses the path
try {
    $si = $installer.GetType().InvokeMember("SummaryInformation", "InvokeMethod", $null, $installer, @("", $msiPath, 1))
    Write-Host "Step 2: Got SummaryInfo via Installer(empty_db, path)"
} catch {
    Write-Host "Empty db approach failed: $_"
    try {
        # Try with just path and updateCount
        $si = $installer.GetType().InvokeMember("SummaryInformation", "InvokeMethod", $null, $installer, @($msiPath, 1))
        Write-Host "Step 2: Got SummaryInfo via Installer(path)"
    } catch {
        Write-Host "Path-only approach also failed: $_"
        $si = $null
    }
}

if ($si -ne $null) {
    $siType = $si.GetType()
    try { $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(1, [int]1252)); Write-Host "  codepage OK" } catch { Write-Host "  codepage: $_" }
    try { $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(2, "Velocity Test")); Write-Host "  title OK" } catch { Write-Host "  title: $_" }
    try { $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(7, "x86;1033")); Write-Host "  template OK" } catch { Write-Host "  template: $_" }
    try { $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(9, $productCode)); Write-Host "  rev OK" } catch { Write-Host "  rev: $_" }
    try { $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(14, [int]405)); Write-Host "  security OK" } catch { Write-Host "  security: $_" }
    try { $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(15, [int]2)); Write-Host "  wordcount OK" } catch { Write-Host "  wordcount: $_" }
    try { $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(18, "Velocity")); Write-Host "  app OK" } catch { Write-Host "  app: $_" }
    try { $si.GetType().InvokeMember("Persist", "InvokeMethod", $null, $si, $null); Write-Host "  persist OK" } catch { Write-Host "  persist: $_" }
} else {
    Write-Host "SummaryInfo is null - skipping"
}

Write-Host "`nCreated: $msiPath ($(((Get-Item $msiPath).Length)) bytes)"

# Test with msiexec
$proc = Start-Process -FilePath "msiexec.exe" -ArgumentList "/i `"$msiPath`" /qn /l*v C:\temp\minimal_valid.log" -Wait -PassThru
Write-Host "msiexec exit code: $($proc.ExitCode)"
