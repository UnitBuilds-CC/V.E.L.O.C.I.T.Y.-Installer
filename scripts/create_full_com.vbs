' Create a complete MSI with SummaryInfo - two phase approach
' Phase 1: Create database with tables and data
Dim installer, db, view, si, fso

Set fso = CreateObject("Scripting.FileSystemObject")
If fso.FileExists("C:\temp\com_full.msi") Then fso.DeleteFile "C:\temp\com_full.msi", True

Set installer = CreateObject("WindowsInstaller.Installer")

' Phase 1: Create database with tables and data
Set db = installer.OpenDatabase("C:\temp\com_full.msi", 3)
WScript.Echo "Phase 1: Database created"

Sub RunSQL(sql)
    Set view = db.OpenView(sql)
    view.Execute
    view.Close
End Sub

' Create tables
RunSQL "CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY `Property`)"
RunSQL "CREATE TABLE `Directory` (`Directory` CHAR(72) NOT NULL, `Directory_Parent` CHAR(72), `DefaultDir` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY `Directory`)"
RunSQL "CREATE TABLE `Component` (`Component` CHAR(72) NOT NULL, `ComponentId` CHAR(38), `Directory_` CHAR(72) NOT NULL, `Attributes` SHORT NOT NULL, `Condition` CHAR(255), `KeyPath` CHAR(72) PRIMARY KEY `Component`)"
RunSQL "CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL, `Feature_Parent` CHAR(38), `Title` CHAR(64) LOCALIZABLE, `Description` CHAR(255) LOCALIZABLE, `Display` SHORT, `Level` SHORT NOT NULL, `Directory_` CHAR(72), `Attributes` SHORT NOT NULL PRIMARY KEY `Feature`)"
RunSQL "CREATE TABLE `FeatureComponents` (`Feature_` CHAR(38) NOT NULL, `Component_` CHAR(72) NOT NULL PRIMARY KEY `Feature_`, `Component_`)"
RunSQL "CREATE TABLE `InstallExecuteSequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)"
RunSQL "CREATE TABLE `InstallUISequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)"
WScript.Echo "Tables created"

Dim guid1, guid2
guid1 = UCase(Left(CreateObject("Scriptlet.TypeLib").Guid, 38))
guid2 = UCase(Left(CreateObject("Scriptlet.TypeLib").Guid, 38))

RunSQL "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductName', 'Velocity Test')"
RunSQL "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductVersion', '1.0.0')"
RunSQL "INSERT INTO `Property` (`Property`, `Value`) VALUES ('Manufacturer', 'Velocity Corp')"
RunSQL "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductCode', '" & guid1 & "')"
RunSQL "INSERT INTO `Property` (`Property`, `Value`) VALUES ('UpgradeCode', '" & guid2 & "')"
RunSQL "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductLanguage', '1033')"

RunSQL "INSERT INTO `Directory` (`Directory`, `Directory_Parent`, `DefaultDir`) VALUES ('TARGETDIR', '', 'SourceDir')"
RunSQL "INSERT INTO `Directory` (`Directory`, `Directory_Parent`, `DefaultDir`) VALUES ('ProgramFilesFolder', 'TARGETDIR', 'PFiles')"
RunSQL "INSERT INTO `Directory` (`Directory`, `Directory_Parent`, `DefaultDir`) VALUES ('INSTALLDIR', 'ProgramFilesFolder', 'VelocityTest')"

RunSQL "INSERT INTO `Component` (`Component`, `Directory_`, `Attributes`) VALUES ('MainComp', 'INSTALLDIR', 0)"
RunSQL "INSERT INTO `Feature` (`Feature`, `Title`, `Level`, `Attributes`) VALUES ('MainFeat', 'Complete', 1, 0)"
RunSQL "INSERT INTO `FeatureComponents` (`Feature_`, `Component_`) VALUES ('MainFeat', 'MainComp')"

RunSQL "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('CostInitialize', 800)"
RunSQL "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('CostFinalize', 1000)"
RunSQL "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('InstallValidate', 1400)"
RunSQL "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('InstallInitialize', 1500)"
RunSQL "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('InstallFiles', 4000)"
RunSQL "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('InstallFinalize', 6600)"

RunSQL "INSERT INTO `InstallUISequence` (`Action`, `Sequence`) VALUES ('CostInitialize', 800)"
RunSQL "INSERT INTO `InstallUISequence` (`Action`, `Sequence`) VALUES ('CostFinalize', 1000)"
RunSQL "INSERT INTO `InstallUISequence` (`Action`, `Sequence`) VALUES ('ExecuteAction', 1300)"
WScript.Echo "All data inserted"

db.Commit
WScript.Echo "Phase 1: Committed"

' Release the database
Set db = Nothing
Set view = Nothing

' Phase 2: Open in transact mode and add SummaryInfo
WScript.Echo "Phase 2: Adding SummaryInfo"
Set db = installer.OpenDatabase("C:\temp\com_full.msi", 1)
WScript.Echo "Database reopened"

Set si = db.SummaryInformation(20)
WScript.Echo "SummaryInformation obtained"

si.Property(1) = "Velocity Test Installation"
WScript.Echo "Title set"
si.Property(2) = "Velocity Test"
si.Property(4) = "Velocity Corp"
si.Property(7) = "Intel;1033"
si.Property(9) = CreateObject("Scriptlet.TypeLib").Guid
si.Property(14) = 0
si.Property(15) = 2
WScript.Echo "All properties set"

si.Persist
WScript.Echo "Persisted"

db.Commit
WScript.Echo "Committed"

Dim size
size = fso.GetFile("C:\temp\com_full.msi").Size
WScript.Echo "Created: C:\temp\com_full.msi (" & size & " bytes)"
