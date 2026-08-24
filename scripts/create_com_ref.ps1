$ErrorActionPreference = "Stop"
$msiPath = "C:\Temp\com_reference.msi"
$logPath = "C:\Temp\com_reference_install.log"

# Clean up
New-Item -ItemType Directory -Force -Path "C:\Temp" | Out-Null
if (Test-Path $msiPath) { Remove-Item $msiPath -Force }
if (Test-Path $logPath) { Remove-Item $logPath -Force }

# Create MSI via WindowsInstaller COM
$installer = New-Object -ComObject WindowsInstaller.Installer
$db = $installer.OpenDatabase($msiPath, 1)

function RunSQL($db, $sql) {
    $view = $db.OpenView($sql)
    $view.Execute()
    $view.Close()
}

# Create tables
RunSQL $db "CREATE TABLE Property (Property CHAR(72) NOT NULL PRIMARY KEY, Value CHAR(255) NULL)"
RunSQL $db "CREATE TABLE Directory (Directory CHAR(72) NOT NULL PRIMARY KEY, Directory_Parent CHAR(72) NULL, DefaultDir CHAR(255) NULL)"
RunSQL $db "CREATE TABLE Component (Component CHAR(72) NOT NULL PRIMARY KEY, ComponentId CHAR(38) NULL, Directory_ CHAR(72) NOT NULL, Attributes SHORT NOT NULL, Condition CHAR(255) NULL, KeyPath CHAR(72) NULL)"
RunSQL $db "CREATE TABLE Feature (Feature CHAR(38) NOT NULL PRIMARY KEY, Feature_Parent CHAR(38) NULL, Title CHAR(64) NULL, Description CHAR(255) NULL, Display SHORT NULL, Level SHORT NOT NULL, Directory_ CHAR(72) NULL, Attributes SHORT NOT NULL)"
RunSQL $db "CREATE TABLE FeatureComponents (Feature_ CHAR(38) NOT NULL PRIMARY KEY, Component_ CHAR(72) NOT NULL PRIMARY KEY)"
RunSQL $db "CREATE TABLE InstallExecuteSequence (Action CHAR(72) NOT NULL PRIMARY KEY, Condition CHAR(255) NULL, Sequence SHORT NULL)"
RunSQL $db "CREATE TABLE InstallUISequence (Action CHAR(72) NOT NULL PRIMARY KEY, Condition CHAR(255) NULL, Sequence SHORT NULL)"

# Properties
$productCode = "{B1234567-1234-1234-1234-123456789ABC}"
$upgradeCode = "{C1234567-1234-1234-1234-123456789ABC}"
RunSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductName', 'COM Reference Test')"
RunSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductVersion', '1.0.0')"
RunSQL $db "INSERT INTO Property (Property, Value) VALUES ('Manufacturer', 'Velocity Team')"
RunSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductCode', '$productCode')"
RunSQL $db "INSERT INTO Property (Property, Value) VALUES ('UpgradeCode', '$upgradeCode')"
RunSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductLanguage', '1033')"

# Directories
RunSQL $db "INSERT INTO Directory (Directory, Directory_Parent, DefaultDir) VALUES ('TARGETDIR', NULL, 'SourceDir')"
RunSQL $db "INSERT INTO Directory (Directory, Directory_Parent, DefaultDir) VALUES ('LocalAppDataFolder', 'TARGETDIR', 'LocalAppData')"
RunSQL $db "INSERT INTO Directory (Directory, Directory_Parent, DefaultDir) VALUES ('INSTALLDIR', 'LocalAppDataFolder', 'ComRef:ComRef')"

# Components (Null GUID)
RunSQL $db "INSERT INTO Component (Component, ComponentId, Directory_, Attributes, Condition, KeyPath) VALUES ('comp_0', NULL, 'INSTALLDIR', 0, NULL, 'file_0')"
RunSQL $db "INSERT INTO Component (Component, ComponentId, Directory_, Attributes, Condition, KeyPath) VALUES ('comp_1', NULL, 'INSTALLDIR', 0, NULL, 'file_1')"

# Feature
RunSQL $db "INSERT INTO Feature (Feature, Feature_Parent, Title, Description, Display, Level, Directory_, Attributes) VALUES ('Complete', NULL, 'Complete', 'All files', 1, 1, 'INSTALLDIR', 0)"

# FeatureComponents
RunSQL $db "INSERT INTO FeatureComponents (Feature_, Component_) VALUES ('Complete', 'comp_0')"
RunSQL $db "INSERT INTO FeatureComponents (Feature_, Component_) VALUES ('Complete', 'comp_1')"

# InstallExecuteSequence
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('LaunchConditions', 'NOT Installed', 100)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('CostInitialize', NULL, 800)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('CostFinalize', NULL, 1000)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('InstallValidate', NULL, 1400)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('InstallInitialize', NULL, 1500)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('ProcessComponents', NULL, 1600)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('RegisterProduct', NULL, 6100)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('PublishFeatures', NULL, 6300)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('PublishProduct', NULL, 6400)"
RunSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('InstallFinalize', NULL, 6600)"

# InstallUISequence
RunSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('LaunchConditions', NULL, 100)"
RunSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('CostInitialize', NULL, 800)"
RunSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('CostFinalize', NULL, 1000)"
RunSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('ExecuteAction', NULL, 1300)"

$db.Commit()

# Set SummaryInformation
$db2 = $installer.OpenDatabase($msiPath, 1)
$si = $db2.SummaryInformation(1)
$si.Property(1) = "COM Reference Test"
$si.Property(2) = "Velocity Team"
$si.Property(3) = "Velocity Team"
$si.Property(4) = "COM reference for velocity-msi debugging"
$si.Property(7) = "x64;1033"
$si.Property(9) = "{D1234567-1234-1234-1234-123456789ABC}"
$si.Property(14) = 405
$si.Property(15) = 2
$si.Persist()
$db2.Commit()

Write-Host "Created: $msiPath"
Write-Host "Size: $((Get-Item $msiPath).Length) bytes"
Write-Host ""
Write-Host "To install:"
Write-Host "  Start-Process msiexec -ArgumentList '/i','$msiPath','/qn','/l*vx','$logPath' -Wait"
Write-Host ""
Write-Host "ProductCode: $productCode"
