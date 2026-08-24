# Create a reference MSI via COM with identical table data to our velocity-msi output.
$outputDir = "C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\examples\sample-app\output"
$msiPath = "$outputDir\com_reference.msi"

# Remove existing
if (Test-Path $msiPath) { Remove-Item $msiPath -Force }

# Create empty file first
[System.IO.File]::WriteAllBytes($msiPath, [byte[]]@())

# Open as new database (mode 3 = msiOpenDatabaseModeCreateDirect)
$wi = New-Object -ComObject WindowsInstaller.Installer
$db = $wi.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $wi, @($msiPath, 3))
Write-Host "Database created"

# Helper to execute SQL
function ExecSQL($db, $sql) {
    $view = $db.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $db, @($sql))
    $view.GetType().InvokeMember("Execute", "InvokeMethod", $null, $view, $null)
    $view.GetType().InvokeMember("Close", "InvokeMethod", $null, $view, $null)
}

# === Create tables ===
ExecSQL $db "CREATE TABLE Property (Property CHAR(72) NOT NULL PRIMARY KEY, Value CHAR(255) NULL)"
ExecSQL $db "CREATE TABLE ``Directory`` (``Directory`` CHAR(72) NOT NULL PRIMARY KEY, ``Directory_Parent`` CHAR(72) NULL, ``DefaultDir`` CHAR(255) NULL)"
ExecSQL $db "CREATE TABLE Component (Component CHAR(72) NOT NULL PRIMARY KEY, ComponentId CHAR(38) NULL, ``Directory_`` CHAR(72) NOT NULL, Attributes INT NOT NULL, Condition CHAR(255) NULL, KeyPath CHAR(72) NULL)"
ExecSQL $db "CREATE TABLE File (File CHAR(72) NOT NULL PRIMARY KEY, Component_ CHAR(72) NOT NULL, FileName CHAR(255) NOT NULL, FileSize INT NOT NULL, Version CHAR(72) NULL, Language CHAR(20) NULL, Attributes INT NULL, Sequence INT NOT NULL)"
ExecSQL $db "CREATE TABLE Media (DiskId INT NOT NULL PRIMARY KEY, LastSequence INT NOT NULL, DiskPrompt CHAR(255) NULL, VolumeLabel CHAR(32) NULL, Cabinet CHAR(255) NULL, Source CHAR(72) NULL)"
ExecSQL $db "CREATE TABLE Feature (Feature CHAR(38) NOT NULL PRIMARY KEY, Feature_Parent CHAR(38) NULL, Title CHAR(64) NULL, Description CHAR(255) NULL, Display INT NULL, Level INT NOT NULL, ``Directory_`` CHAR(72) NULL, Attributes INT NOT NULL)"
ExecSQL $db "CREATE TABLE FeatureComponents (Feature_ CHAR(38) NOT NULL PRIMARY KEY, Component_ CHAR(72) NOT NULL PRIMARY KEY)"
ExecSQL $db "CREATE TABLE InstallExecuteSequence (Action CHAR(72) NOT NULL PRIMARY KEY, Condition CHAR(255) NULL, Sequence INT NULL)"
ExecSQL $db "CREATE TABLE InstallUISequence (Action CHAR(72) NOT NULL PRIMARY KEY, Condition CHAR(255) NULL, Sequence INT NULL)"
Write-Host "Tables created"

# === Populate Property table ===
$productCode = "{F1234567-1234-1234-1234-123456789ABC}"
$upgradeCode = "{11223344-5566-7788-99AA-BBCCDDEEFF00}"

ExecSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductName', 'COM Reference Test')"
ExecSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductVersion', '1.0.0')"
ExecSQL $db "INSERT INTO Property (Property, Value) VALUES ('Manufacturer', 'Velocity Team')"
ExecSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductCode', '$productCode')"
ExecSQL $db "INSERT INTO Property (Property, Value) VALUES ('UpgradeCode', '$upgradeCode')"
ExecSQL $db "INSERT INTO Property (Property, Value) VALUES ('ProductLanguage', '1033')"
Write-Host "Properties inserted"

# === Populate Directory table ===
ExecSQL $db "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('TARGETDIR', NULL, 'SourceDir')"
ExecSQL $db "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('LocalAppDataFolder', 'TARGETDIR', 'LocalAppData')"
ExecSQL $db "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('INSTALLDIR', 'LocalAppDataFolder', 'ComRefTest:ComRefTest')"

# === Populate Component table ===
ExecSQL $db "INSERT INTO Component (Component, ComponentId, ``Directory_``, Attributes, Condition, KeyPath) VALUES ('comp_0', NULL, 'INSTALLDIR', 0, NULL, 'file_0')"
ExecSQL $db "INSERT INTO Component (Component, ComponentId, ``Directory_``, Attributes, Condition, KeyPath) VALUES ('comp_1', NULL, 'INSTALLDIR', 0, NULL, 'file_1')"

# === Populate File table ===
ExecSQL $db "INSERT INTO File (File, Component_, FileName, FileSize, Version, Language, Attributes, Sequence) VALUES ('file_0', 'comp_0', 'hello.txt', 24, NULL, NULL, 0, 1)"
ExecSQL $db "INSERT INTO File (File, Component_, FileName, FileSize, Version, Language, Attributes, Sequence) VALUES ('file_1', 'comp_1', 'data.txt', 42, NULL, NULL, 0, 2)"

# === Populate Media table ===
ExecSQL $db "INSERT INTO Media (DiskId, LastSequence, DiskPrompt, VolumeLabel, Cabinet, Source) VALUES (1, 2, NULL, NULL, '#Velocity.cab', NULL)"

# === Populate Feature table ===
ExecSQL $db "INSERT INTO Feature (Feature, Feature_Parent, Title, Description, Display, Level, ``Directory_``, Attributes) VALUES ('Complete', NULL, 'Complete', 'All files', 1, 1, 'INSTALLDIR', 0)"

# === Populate FeatureComponents ===
ExecSQL $db "INSERT INTO FeatureComponents (Feature_, Component_) VALUES ('Complete', 'comp_0')"
ExecSQL $db "INSERT INTO FeatureComponents (Feature_, Component_) VALUES ('Complete', 'comp_1')"

# === Populate InstallExecuteSequence ===
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('LaunchConditions', 'NOT Installed', 100)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('CostInitialize', NULL, 800)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('FileCost', NULL, 900)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('CostFinalize', NULL, 1000)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('InstallValidate', NULL, 1400)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('InstallInitialize', NULL, 1500)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('ProcessComponents', NULL, 1600)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('InstallFiles', NULL, 4000)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('RegisterProduct', NULL, 6100)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('PublishFeatures', NULL, 6300)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('PublishProduct', NULL, 6400)"
ExecSQL $db "INSERT INTO InstallExecuteSequence (Action, Condition, Sequence) VALUES ('InstallFinalize', NULL, 6600)"

# === Populate InstallUISequence ===
ExecSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('LaunchConditions', NULL, 100)"
ExecSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('CostInitialize', NULL, 800)"
ExecSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('CostFinalize', NULL, 1000)"
ExecSQL $db "INSERT INTO InstallUISequence (Action, Condition, Sequence) VALUES ('ExecuteAction', NULL, 1300)"
Write-Host "All data inserted"

# === Set SummaryInformation properties ===
$si = $db.GetType().InvokeMember("SummaryInformation", "GetProperty", $null, $db, $null)
$siType = $si.GetType()
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(1, [int]1252))    # Codepage
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(2, "COM Reference Test"))  # Title
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(3, "COM Reference MSI"))   # Subject
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(4, "Velocity Team"))       # Author
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(7, "x64;1033"))            # Template
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(9, "{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}"))  # Revision
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(14, [int]405))   # Security
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(15, [int]2))     # WordCount
$siType.InvokeMember("Property", "InvokeMethod", $null, $si, @(18, "Velocity Installer")) # CreatingApp
$siType.InvokeMember("Persist", "InvokeMethod", $null, $si, $null)
Write-Host "SummaryInfo set"

# Commit
$db.GetType().InvokeMember("Commit", "InvokeMethod", $null, $db, $null)
$size = (Get-Item $msiPath).Length
Write-Host "`nCOM reference MSI created: $msiPath ($size bytes)"
Write-Host "ProductCode: $productCode"
