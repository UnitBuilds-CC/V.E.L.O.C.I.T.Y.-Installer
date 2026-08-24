# Create reference MSI using Windows Installer COM API
$installer = New-Object -ComObject WindowsInstaller.Installer

# msiOpenDatabaseModeCreate = 1
$database = $installer.OpenDatabase("C:\temp\ref_com.msi", 1)

# Create Property table
$database.CreateTable("Property")

$record = $installer.CreateRecord(2)
$record.StringData(1) = "ProductName"
$record.StringData(2) = "Test Product"
$database.Execute($record)

$record = $installer.CreateRecord(2)
$record.StringData(1) = "ProductCode"
$record.StringData(2) = "{12345678-1234-1234-1234-123456789012}"
$database.Execute($record)

$record = $installer.CreateRecord(2)
$record.StringData(1) = "ProductVersion"
$record.StringData(2) = "1.0.0"
$database.Execute($record)

$record = $installer.CreateRecord(2)
$record.StringData(1) = "Manufacturer"
$record.StringData(2) = "Test Corp"
$database.Execute($record)

$record = $installer.CreateRecord(2)
$record.StringData(1) = "ProductLanguage"
$record.StringData(2) = "1033"
$database.Execute($record)

$record = $installer.CreateRecord(2)
$record.StringData(1) = "UpgradeCode"
$record.StringData(2) = "{87654321-4321-4321-4321-210987654321}"
$database.Execute($record)

# Create Directory table
$database.CreateTable("Directory")
$record = $installer.CreateRecord(3)
$record.StringData(1) = "TARGETDIR"
$record.StringData(2) = ""
$record.StringData(3) = "."
$database.Execute($record)

$record = $installer.CreateRecord(3)
$record.StringData(1) = "ProgramFilesFolder"
$record.StringData(2) = "TARGETDIR"
$record.StringData(3) = "TestProduct"
$database.Execute($record)

$record = $installer.CreateRecord(3)
$record.StringData(1) = "INSTALLDIR"
$record.StringData(2) = "ProgramFilesFolder"
$record.StringData(3) = "TestProduct"
$database.Execute($record)

# Create Component table
$database.CreateTable("Component")
$record = $installer.CreateRecord(6)
$record.StringData(1) = "MainComponent"
$record.StringData(2) = "{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}"
$record.StringData(3) = "INSTALLDIR"
$record.IntegerData(4) = 256
$record.StringData(5) = ""
$record.StringData(6) = ""
$database.Execute($record)

# Create File table
$database.CreateTable("File")
$record = $installer.CreateRecord(5)
$record.StringData(1) = "test.txt"
$record.StringData(2) = "MainComponent"
$record.StringData(3) = "test.txt"
$record.IntegerData(4) = 13
$record.IntegerData(5) = 0
$database.Execute($record)

# Create Feature table
$database.CreateTable("Feature")
$record = $installer.CreateRecord(8)
$record.StringData(1) = "Complete"
$record.StringData(2) = ""
$record.StringData(3) = "Complete Installation"
$record.IntegerData(4) = 2
$record.IntegerData(5) = 1
$record.StringData(6) = ""
$record.IntegerData(7) = 0
$record.IntegerData(8) = 0
$database.Execute($record)

# Create FeatureComponents table
$database.CreateTable("FeatureComponents")
$record = $installer.CreateRecord(2)
$record.StringData(1) = "Complete"
$record.StringData(2) = "MainComponent"
$database.Execute($record)

# Create Media table
$database.CreateTable("Media")
$record = $installer.CreateRecord(5)
$record.IntegerData(1) = 1
$record.IntegerData(2) = 1
$record.StringData(3) = ""
$record.StringData(4) = ""
$record.StringData(5) = ""
$database.Execute($record)

# Create InstallExecuteSequence table
$database.CreateTable("InstallExecuteSequence")
$record = $installer.CreateRecord(3)
$record.StringData(1) = "InstallValidate"
$record.StringData(2) = ""
$record.IntegerData(3) = 1400
$database.Execute($record)

$record = $installer.CreateRecord(3)
$record.StringData(1) = "InstallInitialize"
$record.StringData(2) = ""
$record.IntegerData(3) = 1500
$database.Execute($record)

$record = $installer.CreateRecord(3)
$record.StringData(1) = "InstallFinalize"
$record.StringData(2) = ""
$record.IntegerData(3) = 6600
$database.Execute($record)

# Set summary info
$summary = $database.SummaryInformation(4)
$summary.Property(1) = "Test Product"
$summary.Property(7) = "Test Corp"
$summary.Property(9) = "{12345678-1234-1234-1234-123456789012}"
$summary.Property(14) = 200
$summary.Property(19) = 0
$summary.Flush()

$database.Commit()
Write-Host "MSI created successfully"
Write-Host "File size: $((Get-Item 'C:\temp\ref_com.msi').Length) bytes"
