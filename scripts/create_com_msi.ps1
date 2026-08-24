# Create a minimal MSI with tables only (no SummaryInfo via COM - that's tricky)
$msiPath = "C:\temp\com_tables_only.msi"
if (Test-Path $msiPath) { Remove-Item $msiPath -Force }

$installer = New-Object -ComObject WindowsInstaller.Installer
$db = $installer.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $installer, @($msiPath, 3))

# Create tables using backtick-escaped names (MSI SQL requires this for reserved words)
$db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @("CREATE TABLE ``Property`` (``Property`` CHAR(72) NOT NULL LOCALIZABLE, ``Value`` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY ``Property``)")) | ForEach-Object { $_.GetType().InvokeMember("Execute", "InvokeMethod", $null, $_, $null) }

$productCode = "{" + [guid]::NewGuid().ToString().ToUpper() + "}"
$upgradeCode = "{" + [guid]::NewGuid().ToString().ToUpper() + "}"

# Insert properties
$inserts = @(
    "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductName', 'Velocity Test')",
    "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductVersion', '1.0.0')",
    "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('Manufacturer', 'Velocity Corp')",
    "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductCode', '$productCode')",
    "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('UpgradeCode', '$upgradeCode')"
)
foreach ($sql in $inserts) {
    $v = $db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @($sql))
    $v.GetType().InvokeMember("Execute", "InvokeMethod", $null, $v, $null)
}

# Set SummaryInfo via reflection
$si = $db.GetType().InvokeMember("SummaryInformation", "InvokeMethod", $null, $db, @(1))
try {
    $si.GetType().InvokeMember("Property", "InvokeMethod", $null, $si, @(1, 1252))
    $si.GetType().InvokeMember("Property", "InvokeMethod", $null, $si, @(2, "Velocity Test"))
    $si.GetType().InvokeMember("Property", "InvokeMethod", $null, $si, @(7, "x86;1033"))
    $si.GetType().InvokeMember("Property", "InvokeMethod", $null, $si, @(9, "{$productCode}"))
    $si.GetType().InvokeMember("Property", "InvokeMethod", $null, $si, @(14, 405))
    $si.GetType().InvokeMember("Property", "InvokeMethod", $null, $si, @(15, 2))
    $si.GetType().InvokeMember("Property", "InvokeMethod", $null, $si, @(18, "Velocity Installer"))
    $si.GetType().InvokeMember("Persist", "InvokeMethod", $null, $si, $null)
    Write-Host "SummaryInfo set OK"
} catch {
    Write-Host "SummaryInfo failed: $_"
}

$db.GetType().InvokeMember("Commit", "InvokeMethod", $null, $db, $null)
Write-Host "Created: $msiPath ($(((Get-Item $msiPath).Length)) bytes)"

# Test with msiexec
$proc = Start-Process -FilePath "msiexec.exe" -ArgumentList "/i `"$msiPath`" /qn /l*v C:\temp\com_tables.log" -Wait -PassThru
Write-Host "msiexec exit code: $($proc.ExitCode)"
