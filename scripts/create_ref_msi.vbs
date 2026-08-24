' Create reference MSI with embedded cabinet using Windows Installer COM API
' Run: cscript create_ref_msi.vbs

Option Explicit

Dim installer, database, summaryInfo, view, record
Set installer = CreateObject("WindowsInstaller.Installer")

' Create new MSI database
Dim msiPath
msiPath = "C:\temp\ref_msi.msi"

' Delete existing file
Dim fso
Set fso = CreateObject("Scripting.FileSystemObject")
If fso.FileExists(msiPath) Then fso.DeleteFile msiPath

' Create empty file first
Dim emptyFile
Set emptyFile = fso.CreateTextFile(msiPath, True)
emptyFile.Close

' Create the database (msiOpenDatabaseModeCreate = 2)
Set database = installer.OpenDatabase(msiPath, 2)

WScript.Echo "Created database: " & msiPath

' Set Summary Information
Set summaryInfo = database.SummaryInformation(1)
summaryInfo.Property(1) = "Reference MSI"           ' Title
summaryInfo.Property(4) = "Velocity Corp"           ' Author
summaryInfo.Property(6) = "Test installation"       ' Subject
summaryInfo.Property(7) = "Velocity Test Product"   ' Template (Intel;1033)
summaryInfo.Property(9) = "{12345678-1234-1234-1234-123456789012}" ' Revision (ProductCode)
summaryInfo.Property(14) = 405                      ' Security

' Create Property table
database.Execute "CREATE TABLE Property (`Property` CHAR(72) NOT NULL, `Value` CHAR(255) LOCALIZABLE NULLABLE PRIMARY KEY `Property`)"

Dim props(5, 1)
props(0, 0) = "ProductName"     : props(0, 1) = "Reference MSI Product"
props(1, 0) = "ProductVersion"  : props(1, 1) = "1.0.0"
props(2, 0) = "Manufacturer"    : props(2, 1) = "Velocity Corp"
props(3, 0) = "ProductCode"     : props(3, 1) = "{12345678-1234-1234-1234-123456789012}"
props(4, 0) = "UpgradeCode"     : props(4, 1) = "{87654321-4321-4321-4321-210987654321}"
props(5, 0) = "ProductLanguage" : props(5, 1) = "1033"

Dim i
For i = 0 To 5
    Set view = database.OpenView("SELECT * FROM Property")
    Set record = installer.CreateRecord(2)
    record.StringData(1) = props(i, 0)
    record.StringData(2) = props(i, 1)
    view.Modify 2, record  ' msidbModifyInsert = 2
    view.Close
Next

' Create Directory table
database.Execute "CREATE TABLE Directory (`Directory` CHAR(72) NOT NULL, `Directory_Parent` CHAR(72) NULLABLE, `DefaultDir` CHAR(255) LOCALIZABLE NULLABLE PRIMARY KEY `Directory`)"

Set view = database.OpenView("SELECT * FROM Directory")

Set record = installer.CreateRecord(3)
record.StringData(1) = "TARGETDIR"
record.StringData(2) = ""
record.StringData(3) = "SourceDir"
view.Modify 2, record

Set record = installer.CreateRecord(3)
record.StringData(1) = "ProgramFilesFolder"
record.StringData(2) = "TARGETDIR"
record.StringData(3) = "PFiles"
view.Modify 2, record

Set record = installer.CreateRecord(3)
record.StringData(1) = "INSTALLDIR"
record.StringData(2) = "ProgramFilesFolder"
record.StringData(3) = "VelRefTest"
view.Modify 2, record
view.Close

' Create Component table
database.Execute "CREATE TABLE Component (`Component` CHAR(72) NOT NULL, `ComponentId` CHAR(38) NULLABLE, `Directory_` CHAR(72) NOT NULL, `Attributes` SHORT NOT NULL, `Condition` CHAR(255) NULLABLE, `KeyPath` CHAR(72) NULLABLE PRIMARY KEY `Component`)"

Set view = database.OpenView("SELECT * FROM Component")
Set record = installer.CreateRecord(6)
record.StringData(1) = "MainComp"
record.StringData(3) = "INSTALLDIR"
record.IntegerData(4) = 0
record.StringData(6) = "MainFile"
view.Modify 2, record
view.Close

' Create Feature table
database.Execute "CREATE TABLE Feature (`Feature` CHAR(38) NOT NULL, `Feature_Parent` CHAR(38) NULLABLE, `Title` CHAR(64) LOCALIZABLE NULLABLE, `Description` CHAR(255) LOCALIZABLE NULLABLE, `Display` SHORT NULLABLE, `Level` SHORT NOT NULL, `Directory_` CHAR(72) NULLABLE, `Attributes` SHORT NOT NULL PRIMARY KEY `Feature`)"

Set view = database.OpenView("SELECT * FROM Feature")
Set record = installer.CreateRecord(8)
record.StringData(1) = "MainFeat"
record.StringData(3) = "Complete"
record.IntegerData(6) = 1
record.IntegerData(8) = 0
view.Modify 2, record
view.Close

' Create FeatureComponents table
database.Execute "CREATE TABLE FeatureComponents (`Feature_` CHAR(38) NOT NULL, `Component_` CHAR(72) NOT NULL PRIMARY KEY `Feature_`, `Component_`)"

Set view = database.OpenView("SELECT * FROM FeatureComponents")
Set record = installer.CreateRecord(2)
record.StringData(1) = "MainFeat"
record.StringData(2) = "MainComp"
view.Modify 2, record
view.Close

' Create File table
database.Execute "CREATE TABLE File (`File_` CHAR(72) NOT NULL, `Component_` CHAR(72) NOT NULL, `FileName` CHAR(255) NOT NULL LOCALIZABLE, `FileSize` LONG NULLABLE, `Sequence` SHORT NOT NULL PRIMARY KEY `File_`)"

Set view = database.OpenView("SELECT * FROM File")
Set record = installer.CreateRecord(5)
record.StringData(1) = "MainFile"
record.StringData(2) = "MainComp"
record.StringData(3) = "testfile.txt"
record.IntegerData(4) = 64
record.IntegerData(5) = 1
view.Modify 2, record
view.Close

' Create InstallExecuteSequence table
database.Execute "CREATE TABLE InstallExecuteSequence (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255) NULLABLE, `Sequence` SHORT NULLABLE PRIMARY KEY `Action`)"

Set view = database.OpenView("SELECT * FROM InstallExecuteSequence")
Dim seqs(5, 1)
seqs(0, 0) = "CostInitialize"   : seqs(0, 1) = "800"
seqs(1, 0) = "FileCost"         : seqs(1, 1) = "900"
seqs(2, 0) = "CostFinalize"     : seqs(2, 1) = "1000"
seqs(3, 0) = "InstallValidate"  : seqs(3, 1) = "1400"
seqs(4, 0) = "InstallInitialize" : seqs(4, 1) = "1500"
seqs(5, 0) = "InstallFinalize"  : seqs(5, 1) = "6600"

For i = 0 To 5
    Set record = installer.CreateRecord(3)
    record.StringData(1) = seqs(i, 0)
    record.IntegerData(3) = CInt(seqs(i, 1))
    view.Modify 2, record
Next
view.Close

' Create InstallUISequence table
database.Execute "CREATE TABLE InstallUISequence (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255) NULLABLE, `Sequence` SHORT NULLABLE PRIMARY KEY `Action`)"

Set view = database.OpenView("SELECT * FROM InstallUISequence")
Set record = installer.CreateRecord(3)
record.StringData(1) = "CostInitialize"
record.IntegerData(3) = 800
view.Modify 2, record
Set record = installer.CreateRecord(3)
record.StringData(1) = "CostFinalize"
record.IntegerData(3) = 1000
view.Modify 2, record
Set record = installer.CreateRecord(3)
record.StringData(1) = "ExecuteAction"
record.IntegerData(3) = 1300
view.Modify 2, record
view.Close

' Create Media table
database.Execute "CREATE TABLE Media (`DiskId` SHORT NOT NULL, `LastSequence` SHORT NOT NULL, `DiskPrompt` CHAR(64) LOCALIZABLE NULLABLE, `Cabinet` CHAR(255) NULLABLE, `VolumeLabel` CHAR(32) LOCALIZABLE NULLABLE, `Source` CHAR(72) NULLABLE PRIMARY KEY `DiskId`)"

Set view = database.OpenView("SELECT * FROM Media")
Set record = installer.CreateRecord(6)
record.IntegerData(1) = 1
record.IntegerData(2) = 1
record.StringData(4) = "#refcab.cab"
view.Modify 2, record
view.Close

' Now add the cabinet as a storage/stream
' For embedded cabinets, the stream name includes the # prefix
' Use the _Storages table or direct stream access

' Actually, we need to use the MSI API to add a binary stream
' The simplest way is to use the _Streams table (deprecated) or
' use the DirectAccess method

' Let's use the Windows Installer automation to add the cabinet stream
' We need to insert into the _Streams table
On Error Resume Next
database.Execute "CREATE TABLE _Streams (`Name` CHAR(62) NOT NULL PRIMARY KEY `Name`, `Data` OBJECT NOT NULL)"

Set view = database.OpenView("SELECT * FROM _Streams")
Set record = installer.CreateRecord(2)
record.StringData(1) = "#refcab.cab"

' Read cabinet file and set as stream data
Dim cabPath
cabPath = "C:\temp\good.cab"
Dim stream
Set stream = fso.OpenTextFile(cabPath, 1, False, 0)  ' ForReading
' Can't read binary with OpenTextFile, need ADODB.Stream
Dim adoStream
Set adoStream = CreateObject("ADODB.Stream")
adoStream.Type = 1  ' adTypeBinary
adoStream.Open
adoStream.LoadFromFile cabPath
record.SetStream 2, adoStream.Read
adoStream.Close

view.Modify 2, record
view.Close
On Error GoTo 0

' Commit changes
summaryInfo.Persist
database.Commit

WScript.Echo "Database committed successfully"

' Now list all streams in the MSI
' Open the database and enumerate streams
Set database = installer.OpenDatabase(msiPath, 0)  ' Open in read-only mode

' Try to read _Streams table to see stream names
On Error Resume Next
Set view = database.OpenView("SELECT * FROM _Streams")
If Err.Number = 0 Then
    view.Execute
    WScript.Echo "Streams in MSI:"
    Do
        Set record = view.Fetch
        If record Is Nothing Then Exit Do
        WScript.Echo "  Stream: '" & record.StringData(1) & "'"
    Loop
    view.Close
Else
    WScript.Echo "Could not open _Streams table: " & Err.Description
End If
On Error GoTo 0

WScript.Echo "Done! MSI created at: " & msiPath
