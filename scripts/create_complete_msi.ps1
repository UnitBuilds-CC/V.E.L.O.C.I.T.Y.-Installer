# Create a complete installable MSI using Windows Installer COM API
$ErrorActionPreference = "Stop"

$msiPath = "C:\temp\com_created.msi"
if (Test-Path $msiPath) { Remove-Item $msiPath -Force }

# Create test files
$srcDir = "C:\temp\com_test_src"
if (!(Test-Path $srcDir)) { New-Item -ItemType Directory -Path $srcDir -Force | Out-Null }
Set-Content -Path "$srcDir\hello.txt" -Value "Hello from Velocity Installer COM!"
Set-Content -Path "$srcDir\readme.md" -Value "# Velocity`nA fast installer."

# Create cabinet
$cabPath = "C:\temp\com_data.cab"
if (Test-Path $cabPath) { Remove-Item $cabPath -Force }
$ddfPath = "C:\temp\com_cabinet.ddf"
$ddfContent = ".Set CabinetName1=com_data.cab`r`n.Set DiskDirectory1=C:\temp`r`n.Set CompressionType=MSZIP`r`n.Set Compress=ON`r`n`"$srcDir\hello.txt`"`r`n`"$srcDir\readme.md`"`r`n"
Set-Content -Path $ddfPath -Value $ddfContent -NoNewline
$null = & makecab /f $ddfPath 2>&1
Remove-Item $ddfPath -Force -ErrorAction SilentlyContinue
Remove-Item "C:\temp\setup.inf" -Force -ErrorAction SilentlyContinue
Remove-Item "C:\temp\setup.rpt" -Force -ErrorAction SilentlyContinue

if (!(Test-Path $cabPath)) { Write-Host "Cabinet creation failed!"; exit 1 }
Write-Host "Cabinet: $((Get-Item $cabPath).Length) bytes"

# Backtick character for MSI SQL
$bt = [char]96  # backtick

# Create MSI via COM
$installer = New-Object -ComObject WindowsInstaller.Installer
$db = $installer.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $installer, @($msiPath, 3))

function Run-SQL($sql) {
    try {
        $v = $db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @($sql))
        $v.GetType().InvokeMember("Execute", "InvokeMethod", $null, $v, $null)
        $v.GetType().InvokeMember("Close", "InvokeMethod", $null, $v, $null)
    } catch {
        Write-Host "SQL FAILED: $sql"
        Write-Host "  Error: $_"
        throw
    }
}

# Create tables using backtick char
Run-SQL "CREATE TABLE ${bt}Property${bt} (${bt}Property${bt} CHAR(72) NOT NULL LOCALIZABLE, ${bt}Value${bt} CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Property${bt})"
Run-SQL "CREATE TABLE ${bt}Directory${bt} (${bt}Directory${bt} CHAR(72) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Directory${bt}, ${bt}Directory_Parent${bt} CHAR(72) LOCALIZABLE, ${bt}DefaultDir${bt} CHAR(255) NOT NULL LOCALIZABLE)"
Run-SQL "CREATE TABLE ${bt}Component${bt} (${bt}Component${bt} CHAR(72) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Component${bt}, ${bt}ComponentId${bt} CHAR(38) LOCALIZABLE, ${bt}Directory_${bt} CHAR(72) NOT NULL LOCALIZABLE, ${bt}Attributes${bt} SHORT, ${bt}Condition${bt} CHAR(255) LOCALIZABLE, ${bt}KeyPath${bt} CHAR(72) LOCALIZABLE)"
Run-SQL "CREATE TABLE ${bt}File${bt} (${bt}File${bt} CHAR(72) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}File${bt}, ${bt}Component_${bt} CHAR(72) NOT NULL LOCALIZABLE, ${bt}FileName${bt} CHAR(255) NOT NULL LOCALIZABLE, ${bt}FileSize${bt} LONG NOT NULL, ${bt}Attributes${bt} SHORT, ${bt}Sequence${bt} SHORT NOT NULL)"
Run-SQL "CREATE TABLE ${bt}Feature${bt} (${bt}Feature${bt} CHAR(38) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Feature${bt}, ${bt}Feature_Parent${bt} CHAR(38) LOCALIZABLE, ${bt}Title${bt} CHAR(64) LOCALIZABLE, ${bt}Description${bt} CHAR(255) LOCALIZABLE, ${bt}Display${bt} SHORT, ${bt}Level${bt} SHORT NOT NULL, ${bt}Directory_${bt} CHAR(72) LOCALIZABLE, ${bt}Attributes${bt} SHORT)"
Run-SQL "CREATE TABLE ${bt}FeatureComponents${bt} (${bt}Feature_${bt} CHAR(38) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Feature_${bt}, ${bt}Component_${bt} CHAR(72) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Component_${bt})"
Run-SQL "CREATE TABLE ${bt}Media${bt} (${bt}DiskId${bt} SHORT NOT NULL PRIMARY KEY ${bt}DiskId${bt}, ${bt}LastSequence${bt} SHORT NOT NULL, ${bt}Cabinet${bt} CHAR(255), ${bt}VolumeLabel${bt} CHAR(32), ${bt}Source${bt} CHAR(72))"
Run-SQL "CREATE TABLE ${bt}InstallExecuteSequence${bt} (${bt}Action${bt} CHAR(72) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Action${bt}, ${bt}Condition${bt} CHAR(255) LOCALIZABLE, ${bt}Sequence${bt} SHORT)"
Run-SQL "CREATE TABLE ${bt}InstallUISequence${bt} (${bt}Action${bt} CHAR(72) NOT NULL LOCALIZABLE PRIMARY KEY ${bt}Action${bt}, ${bt}Condition${bt} CHAR(255) LOCALIZABLE, ${bt}Sequence${bt} SHORT)"
Write-Host "Tables created"

# GUIDs
$productCode = [guid]::NewGuid().ToString().ToUpper()
$upgradeCode = [guid]::NewGuid().ToString().ToUpper()
$componentId = [guid]::NewGuid().ToString().ToUpper()

# Insert Property rows
Run-SQL "INSERT INTO ${bt}Property${bt} (${bt}Property${bt}, ${bt}Value${bt}) VALUES ('ProductName', 'Velocity Test App')"
Run-SQL "INSERT INTO ${bt}Property${bt} (${bt}Property${bt}, ${bt}Value${bt}) VALUES ('ProductCode', '{$productCode}')"
Run-SQL "INSERT INTO ${bt}Property${bt} (${bt}Property${bt}, ${bt}Value${bt}) VALUES ('ProductVersion', '1.0.0')"
Run-SQL "INSERT INTO ${bt}Property${bt} (${bt}Property${bt}, ${bt}Value${bt}) VALUES ('Manufacturer', 'Velocity Team')"
Run-SQL "INSERT INTO ${bt}Property${bt} (${bt}Property${bt}, ${bt}Value${bt}) VALUES ('ProductLanguage', '1033')"
Run-SQL "INSERT INTO ${bt}Property${bt} (${bt}Property${bt}, ${bt}Value${bt}) VALUES ('UpgradeCode', '{$upgradeCode}')"

# Insert Directory rows
Run-SQL "INSERT INTO ${bt}Directory${bt} (${bt}Directory${bt}, ${bt}Directory_Parent${bt}, ${bt}DefaultDir${bt}) VALUES ('TARGETDIR', NULL, '.')"
Run-SQL "INSERT INTO ${bt}Directory${bt} (${bt}Directory${bt}, ${bt}Directory_Parent${bt}, ${bt}DefaultDir${bt}) VALUES ('ProgramFilesFolder', 'TARGETDIR', 'PFiles')"
Run-SQL "INSERT INTO ${bt}Directory${bt} (${bt}Directory${bt}, ${bt}Directory_Parent${bt}, ${bt}DefaultDir${bt}) VALUES ('INSTALLDIR', 'ProgramFilesFolder', 'VelocityTest')"

# Insert Component
Run-SQL "INSERT INTO ${bt}Component${bt} (${bt}Component${bt}, ${bt}ComponentId${bt}, ${bt}Directory_${bt}, ${bt}Attributes${bt}, ${bt}KeyPath${bt}) VALUES ('MainComponent', '{$componentId}', 'INSTALLDIR', 0, 'hello.txt')"

# Insert Files
$helloSize = (Get-Item "$srcDir\hello.txt").Length
$readmeSize = (Get-Item "$srcDir\readme.md").Length
Run-SQL "INSERT INTO ${bt}File${bt} (${bt}File${bt}, ${bt}Component_${bt}, ${bt}FileName${bt}, ${bt}FileSize${bt}, ${bt}Attributes${bt}, ${bt}Sequence${bt}) VALUES ('hello.txt', 'MainComponent', 'hello.txt', $helloSize, 0, 1)"
Run-SQL "INSERT INTO ${bt}File${bt} (${bt}File${bt}, ${bt}Component_${bt}, ${bt}FileName${bt}, ${bt}FileSize${bt}, ${bt}Attributes${bt}, ${bt}Sequence${bt}) VALUES ('readme.md', 'MainComponent', 'readme.md', $readmeSize, 0, 2)"

# Insert Feature
Run-SQL "INSERT INTO ${bt}Feature${bt} (${bt}Feature${bt}, ${bt}Title${bt}, ${bt}Description${bt}, ${bt}Level${bt}, ${bt}Directory_${bt}) VALUES ('Complete', 'Complete Installation', 'Install all features', 1, 'INSTALLDIR')"

# Insert FeatureComponents
Run-SQL "INSERT INTO ${bt}FeatureComponents${bt} (${bt}Feature_${bt}, ${bt}Component_${bt}) VALUES ('Complete', 'MainComponent')"

# Insert Media (embedded cabinet with # prefix)
Run-SQL "INSERT INTO ${bt}Media${bt} (${bt}DiskId${bt}, ${bt}LastSequence${bt}, ${bt}Cabinet${bt}) VALUES (1, 2, '#com_data.cab')"

# InstallExecuteSequence
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('LaunchConditions', NULL, 100)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('ValidateProductID', NULL, 700)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('CostFinalize', NULL, 1000)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('InstallValidate', NULL, 1400)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('InstallInitialize', NULL, 1500)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('ProcessComponents', NULL, 1600)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('UnpublishComponents', NULL, 1700)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('UnpublishFeatures', NULL, 1800)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('RegisterProduct', NULL, 5700)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('PublishFeatures', NULL, 6300)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('PublishProduct', NULL, 6400)"
Run-SQL "INSERT INTO ${bt}InstallExecuteSequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('InstallFinalize', NULL, 6600)"

# InstallUISequence
Run-SQL "INSERT INTO ${bt}InstallUISequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('LaunchConditions', NULL, 100)"
Run-SQL "INSERT INTO ${bt}InstallUISequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('ValidateProductID', NULL, 700)"
Run-SQL "INSERT INTO ${bt}InstallUISequence${bt} (${bt}Action${bt}, ${bt}Condition${bt}, ${bt}Sequence${bt}) VALUES ('CostFinalize', NULL, 1000)"
Write-Host "Data inserted"

# Commit
$db.GetType().InvokeMember("Commit", "InvokeMethod", $null, $db, $null)
Write-Host "Database committed"

# SummaryInfo
$si = $installer.GetType().InvokeMember("SummaryInformation", "InvokeMethod", $null, $installer, @("", $msiPath, 1))
$siType = $si.GetType()
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(1, [int]1252))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(2, "Velocity Test App"))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(3, "Velocity Test"))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(4, "Velocity Team"))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(7, "x64;1033"))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(9, "{$productCode}"))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(14, [int]405))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(15, [int]2))
$null = $siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(18, "Velocity Installer"))
$null = $si.GetType().InvokeMember("Persist", "InvokeMethod", $null, $si, $null)
Write-Host "SummaryInfo set"

Write-Host "`nMSI: $msiPath ($(((Get-Item $msiPath).Length)) bytes)"
Write-Host "ProductCode: {$productCode}"

# Test with msiexec (will fail without cabinet, but tests MSI structure validity)
$installDir = "C:\Program Files\VelocityTest"
if (Test-Path $installDir) { Remove-Item $installDir -Recurse -Force -ErrorAction SilentlyContinue }
$prodBraced = "{$productCode}"
$null = & msiexec /x $prodBraced /qn 2>&1
Start-Sleep -Seconds 1

Write-Host "`n--- msiexec install test ---"
$proc = Start-Process -FilePath "msiexec.exe" -ArgumentList "/i `"$msiPath`" /qn /l*v C:\temp\com_created.log" -Wait -PassThru
Write-Host "Exit code: $($proc.ExitCode)"

if (Test-Path "C:\temp\com_created.log") {
    $log = Get-Content "C:\temp\com_created.log"
    $errors = $log | Where-Object { $_ -match "Error|2219|return value 3|cabinet|Cabinet" }
    if ($errors) {
        Write-Host "`nLog highlights:"
        $errors | Select-Object -First 15 | ForEach-Object { Write-Host "  $_" }
    }
}

if (Test-Path "$installDir\hello.txt") {
    Write-Host "`nFILE: hello.txt INSTALLED!"
} else {
    Write-Host "`nFILE: hello.txt NOT found (expected - no cabinet embedded yet)"
}

Write-Host "`n=== DONE ==="
