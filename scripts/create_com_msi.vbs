Option Explicit

Dim wi, db, view, si, fso, rec
Set fso = CreateObject("Scripting.FileSystemObject")
Set wi = CreateObject("WindowsInstaller.Installer")

' Delete old file
If fso.FileExists("C:\temp\com_ref.msi") Then fso.DeleteFile "C:\temp\com_ref.msi"

' Create new database (mode 3 = create + transact)
Set db = wi.OpenDatabase("C:\temp\com_ref.msi", 3)
WScript.Echo "Database created"

' Create Property table - try without backticks
On Error Resume Next
Set view = db.OpenView("CREATE TABLE Property (Property CHAR(72) NOT NULL, Value CHAR(255))")
view.Execute
If Err.Number <> 0 Then
    WScript.Echo "CREATE TABLE attempt 1 failed: " & Err.Description
    Err.Clear
    
    ' Try with msiCreateTransformFile or alternative approach
    Set view = db.OpenView("CREATE TABLE `Property` (`Property` CHAR(72), `Value` CHAR(255))")
    view.Execute
    If Err.Number <> 0 Then
        WScript.Echo "CREATE TABLE attempt 2 failed: " & Err.Description
        Err.Clear
        
        ' Try with LONG for Value
        Set view = db.OpenView("CREATE TABLE Property (Property CHAR(72), Value CHAR(255))")
        view.Execute
        If Err.Number <> 0 Then
            WScript.Echo "CREATE TABLE attempt 3 failed: " & Err.Description
            Err.Clear
            WScript.Echo "All CREATE TABLE attempts failed"
            WScript.Quit 1
        End If
    End If
End If
On Error GoTo 0
WScript.Echo "Property table created"

' Insert properties using Record object
Dim propNames, propValues
propNames = Array("ProductName", "ProductCode", "ProductVersion", "Manufacturer", "UpgradeCode", "ALLUSERS", "ProductLanguage")
propValues = Array("Test Product", "{A0A0A0A0-B1B1-42C2-83D3-E4E4E4E4E4E4}", "1.0.0", "Test Mfg", "{B1B1B1B1-C2C2-43D3-94E4-F5F5F5F5F5F5}", "1", "1033")

Dim i
For i = 0 To 6
    Set rec = wi.CreateRecord(2)
    rec.StringData(1) = propNames(i)
    rec.StringData(2) = propValues(i)
    Set view = db.OpenView("INSERT INTO Property (Property, Value) VALUES (?, ?)")
    view.Execute rec
Next
WScript.Echo "Properties inserted"

db.Commit
WScript.Echo "Database committed"

' Set SummaryInfo
Set si = db.SummaryInformation(0)
si.Property(1) = 1252     ' Codepage
si.Property(2) = "Installation Database"  ' Title
si.Property(4) = "Test Author"  ' Author
si.Property(7) = "x64;1033"  ' Template
si.Property(9) = "{A0A0A0A0-B1B1-42C2-83D3-E4E4E4E4E4E4}"  ' UUID
si.Property(15) = 2   ' WordCount
si.Property(18) = "Velocity Installer"  ' CreatingApp
si.Persist
db.Commit
WScript.Echo "SummaryInfo committed"

WScript.Echo "COM MSI created at C:\temp\com_ref.msi"
WScript.Echo "File size: " & fso.GetFile("C:\temp\com_ref.msi").Size & " bytes"
