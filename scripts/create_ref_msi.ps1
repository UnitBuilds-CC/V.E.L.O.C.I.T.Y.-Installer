# Create reference MSI with embedded cabinet using COM
# Then examine the OLE stream names

$ErrorActionPreference = "Stop"

# Create a simple test file
$content = "Hello from Reference MSI!"
[System.IO.File]::WriteAllText("C:\temp\reftest.txt", $content)

# Create cabinet using makecab
$ddfContent = @'
.OPTION EXPLICIT
.Set CabinetName1=C:\temp\refcab.cab
.Set DiskDirectory1=C:\temp
.Set CompressionType=MSZIP
.Set Cabinet=on
.Set Compress=on
"C:\temp\reftest.txt"
'@
[System.IO.File]::WriteAllText("C:\temp\ref.ddf", $ddfContent)

# Run makecab
$proc = Start-Process -FilePath "makecab" -ArgumentList "/f", "C:\temp\ref.ddf" -Wait -NoNewWindow -PassThru -RedirectStandardOutput "C:\temp\makecab_out.txt" -RedirectStandardError "C:\temp\makecab_err.txt"
Write-Host "makecab exit: $($proc.ExitCode)"
if (Test-Path "C:\temp\makecab_out.txt") { Get-Content "C:\temp\makecab_out.txt" }
if (Test-Path "C:\temp\makecab_err.txt") { Get-Content "C:\temp\makecab_err.txt" }

if (!(Test-Path "C:\temp\refcab.cab")) {
    Write-Host "ERROR: Cabinet not created!"
    exit 1
}
Write-Host "Cabinet: $((Get-Item C:\temp\refcab.cab).Length) bytes"

# Create MSI using Windows Installer COM
$installer = New-Object -ComObject WindowsInstaller.Installer

# Delete existing
if (Test-Path "C:\temp\ref.msi") { Remove-Item "C:\temp\ref.msi" -Force }

$database = $installer.CreateDatabase("C:\temp\ref.msi", 0)

# Helper to run query
function Q($db, $sql) {
    $v = $db.OpenView($sql)
    $v.Execute() | Out-Null
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($v) | Out-Null
}

# Create tables
Q $database "CREATE TABLE Property (Property CHAR(72) NOT NULL, Value CHAR(255) NULL LOCALIZABLE PRIMARY KEY Property)"
Q $database "CREATE TABLE Directory (Directory CHAR(72) NOT NULL, Directory_Parent CHAR(72) NULL, DefaultDir CHAR(255) NULL LOCALIZABLE PRIMARY KEY Directory)"
Q $database "CREATE TABLE Component (Component CHAR(72) NOT NULL, ComponentId CHAR(38) NULL, Directory_ CHAR(72) NOT NULL, Attributes SHORT NOT NULL, Condition CHAR(255) NULL, KeyPath CHAR(72) NULL PRIMARY KEY Component)"
Q $database "CREATE TABLE Feature (Feature CHAR(38) NOT NULL, Feature_Parent CHAR(38) NULL, Title CHAR(64) NULL LOCALIZABLE, Description CHAR(255) NULL LOCALIZABLE, Display SHORT NULL, Level SHORT NOT NULL, Directory_ CHAR(72) NULL, Attributes SHORT NOT NULL PRIMARY KEY Feature)"
Q $database "CREATE TABLE FeatureComponents (Feature_ CHAR(38) NOT NULL, Component_ CHAR(72) NOT NULL PRIMARY KEY Feature_, Component_)"
Q $database "CREATE TABLE File (File_ CHAR(72) NOT NULL, Component_ CHAR(72) NOT NULL, FileName CHAR(255) NOT NULL LOCALIZABLE, FileSize LONG NULL, Sequence SHORT NOT NULL PRIMARY KEY File_)"
Q $database "CREATE TABLE Media (DiskId SHORT NOT NULL, LastSequence SHORT NOT NULL, DiskPrompt CHAR(64) NULL LOCALIZABLE, Cabinet CHAR(255) NULL, VolumeLabel CHAR(32) NULL LOCALIZABLE, Source CHAR(72) NULL PRIMARY KEY DiskId)"
Q $database "CREATE TABLE InstallExecuteSequence (Action CHAR(72) NOT NULL, Condition CHAR(255) NULL, Sequence SHORT NULL PRIMARY KEY Action)"

# Insert data
$pc = [guid]::NewGuid().ToString('B').ToUpper()
$uc = [guid]::NewGuid().ToString('B').ToUpper()
Write-Host "ProductCode: $pc"

Q $database "INSERT INTO Property (Property, Value) VALUES ('ProductName', 'Ref Product')"
Q $database "INSERT INTO Property (Property, Value) VALUES ('ProductVersion', '1.0.0')"
Q $database "INSERT INTO Property (Property, Value) VALUES ('Manufacturer', 'Ref Corp')"
Q $database "INSERT INTO Property (Property, Value) VALUES ('ProductCode', '$pc')"
Q $database "INSERT INTO Property (Property, Value) VALUES ('UpgradeCode', '$uc')"
Q $database "INSERT INTO Property (Property, Value) VALUES ('ProductLanguage', '1033')"

Q $database "INSERT INTO Directory (Directory, Directory_Parent, DefaultDir) VALUES ('TARGETDIR', '', 'SourceDir')"
Q $database "INSERT INTO Directory (Directory, Directory_Parent, DefaultDir) VALUES ('ProgramFilesFolder', 'TARGETDIR', 'PFiles')"
Q $database "INSERT INTO Directory (Directory, Directory_Parent, DefaultDir) VALUES ('INSTALLDIR', 'ProgramFilesFolder', 'RefTest')"

Q $database "INSERT INTO Component (Component, ComponentId, Directory_, Attributes, KeyPath) VALUES ('MainComp', '', 'INSTALLDIR', 0, 'MainFile')"
Q $database "INSERT INTO Feature (Feature, Title, Level, Attributes) VALUES ('MainFeat', 'Complete', 1, 0)"
Q $database "INSERT INTO FeatureComponents (Feature_, Component_) VALUES ('MainFeat', 'MainComp')"
Q $database "INSERT INTO File (File_, Component_, FileName, FileSize, Sequence) VALUES ('MainFile', 'MainComp', 'reftest.txt', 25, 1)"

Q $database "INSERT INTO InstallExecuteSequence (Action, Sequence) VALUES ('CostInitialize', 800)"
Q $database "INSERT INTO InstallExecuteSequence (Action, Sequence) VALUES ('FileCost', 900)"
Q $database "INSERT INTO InstallExecuteSequence (Action, Sequence) VALUES ('CostFinalize', 1000)"
Q $database "INSERT INTO InstallExecuteSequence (Action, Sequence) VALUES ('InstallValidate', 1400)"
Q $database "INSERT INTO InstallExecuteSequence (Action, Sequence) VALUES ('InstallInitialize', 1500)"
Q $database "INSERT INTO InstallExecuteSequence (Action, Sequence) VALUES ('InstallFinalize', 6600)"

# Media with embedded cabinet
Q $database "INSERT INTO Media (DiskId, LastSequence, Cabinet) VALUES (1, 1, '#refcab.cab')"

# Add cabinet stream using _Streams table
# First, we need to create the _Streams table
try {
    Q $database "CREATE TABLE _Streams (Name CHAR(255) NOT NULL PRIMARY KEY Name, Data OBJECT NOT NULL)"
    Write-Host "Created _Streams table"
} catch {
    Write-Host "_Streams table may already exist"
}

# Insert the cabinet stream
$view = $database.OpenView("INSERT INTO _Streams (Name, Data) VALUES ('refcab.cab', ?)")
$record = $installer.CreateRecord(1)
$record.SetStream(1, "C:\temp\refcab.cab")
$view.Execute($record) | Out-Null
Write-Host "Cabinet stream added via _Streams table"

$database.Commit()

# Summary info
$summary = $database.GetType().InvokeMember("SummaryInformation", [System.Reflection.BindingFlags]::GetProperty, $null, $database, $null)

[System.Runtime.InteropServices.Marshal]::ReleaseComObject($database) | Out-Null
[System.Runtime.InteropServices.Marshal]::ReleaseComObject($installer) | Out-Null

Write-Host "`nReference MSI created: C:\temp\ref.msi"
Write-Host "Testing install..."

# Test install
$proc = Start-Process -FilePath "msiexec" -ArgumentList "/i", "C:\temp\ref.msi", "/qn", "/l*v", "C:\temp\ref.log" -Wait -NoNewWindow -PassThru
Write-Host "Install exit code: $($proc.ExitCode)"

if ($proc.ExitCode -eq 0) {
    Write-Host "SUCCESS! Reference MSI installed!"
    # Uninstall
    $proc2 = Start-Process -FilePath "msiexec" -ArgumentList "/x", $pc, "/qn" -Wait -NoNewWindow -PassThru
    Write-Host "Uninstall exit code: $($proc2.ExitCode)"
} else {
    Write-Host "Install failed. Checking log..."
    if (Test-Path "C:\temp\ref.log") {
        Get-Content "C:\temp\ref.log" | Select-String -Pattern "2725|cabinet|Cabinet|Error" | Select-Object -First 10
    }
}
