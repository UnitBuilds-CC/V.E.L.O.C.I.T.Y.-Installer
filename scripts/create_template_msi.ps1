# Create a minimal valid MSI using Windows Installer COM API
$msiPath = "C:\temp\minimal_template.msi"
if (Test-Path $msiPath) { Remove-Item $msiPath -Force }

$installer = New-Object -ComObject WindowsInstaller.Installer
$database = $installer.OpenDatabase($msiPath, 3)  # mode 3 = create

# Create Property table
$view = $database.OpenView("CREATE TABLE Property (Property CHAR(72) NOT NULL, Value CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY Property)")
$view.Execute()
$view.Close()

# Insert required properties
$productCode = "{" + [guid]::NewGuid().ToString().ToUpper() + "}"
$upgradeCode = "{" + [guid]::NewGuid().ToString().ToUpper() + "}"

$props = @(
    @("ProductName", "Velocity Test"),
    @("ProductVersion", "1.0.0"),
    @("Manufacturer", "Velocity Corp"),
    @("ProductCode", $productCode),
    @("UpgradeCode", $upgradeCode)
)
foreach ($p in $props) {
    $view = $database.OpenView("INSERT INTO Property (Property, Value) VALUES ('$($p[0])', '$($p[1])')")
    $view.Execute()
    $view.Close()
}

# Set summary information using InvokeMember (COM Property setter)
$si = $database.SummaryInformation(0)
$siType = $si.GetType()

function Set-SIProperty($siObj, $siTypeObj, $propId, $value) {
    $siTypeObj.InvokeMember("Property", [System.Reflection.BindingFlags]::SetProperty, $null, $siObj, @($propId, $value)) | Out-Null
}

Set-SIProperty $si $siType 1 1252        # Codepage
Set-SIProperty $si $siType 2 "Velocity Test Installer"  # Title
Set-SIProperty $si $siType 7 "x86;1033"  # Template
Set-SIProperty $si $siType 9 ("{" + [guid]::NewGuid().ToString().ToUpper() + "}")  # RevNumber
Set-SIProperty $si $siType 14 405        # Security
Set-SIProperty $si $siType 15 2          # WordCount
Set-SIProperty $si $siType 18 "Velocity Installer"  # CreatingApp
$si.Persist()

$database.Commit()

Write-Host "Created: $msiPath ($(((Get-Item $msiPath).Length)) bytes)"

# Test with msiexec
$proc = Start-Process -FilePath "msiexec.exe" -ArgumentList "/i `"$msiPath`" /qn /l*v C:\temp\template_test.log" -Wait -PassThru
Write-Host "msiexec exit code: $($proc.ExitCode)"
