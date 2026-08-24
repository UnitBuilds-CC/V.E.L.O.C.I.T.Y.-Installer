Option Explicit

Dim wi, db, view, rec, fso
Set fso = CreateObject("Scripting.FileSystemObject")
Set wi = CreateObject("WindowsInstaller.Installer")

Dim msiPath
msiPath = "C:\temp\diag_msi.msi"

If Not fso.FileExists(msiPath) Then
    WScript.Echo "MSI not found at " & msiPath
    WScript.Quit 1
End If

WScript.Echo "MSI file size: " & fso.GetFile(msiPath).Size & " bytes"

' Try to open the MSI in read-only mode (mode 0)
On Error Resume Next
Set db = wi.OpenDatabase(msiPath, 0)
If Err.Number <> 0 Then
    WScript.Echo "ERROR opening MSI: " & Err.Description & " (0x" & Hex(Err.Number) & ")"
    
    ' Try to get SummaryInfo
    On Error GoTo 0
    On Error Resume Next
    Set db = wi.OpenDatabase(msiPath, 2)  ' msoOpenDatabaseModeDirect
    If Err.Number <> 0 Then
        WScript.Echo "ERROR with direct mode too: " & Err.Description
        WScript.Quit 1
    End If
End If
On Error GoTo 0

WScript.Echo "Database opened successfully!"

' Try to read SummaryInfo
On Error Resume Next
Dim si
Set si = db.SummaryInformation(0)
If Err.Number <> 0 Then
    WScript.Echo "ERROR reading SummaryInfo: " & Err.Description
Else
    WScript.Echo "Title: " & si.Property(2)
    WScript.Echo "Author: " & si.Property(4) 
    WScript.Echo "Template: " & si.Property(7)
    WScript.Echo "Codepage: " & si.Property(1)
End If
On Error GoTo 0

' Try to query Property table
On Error Resume Next
Set view = db.OpenView("SELECT * FROM Property")
If Err.Number <> 0 Then
    WScript.Echo "ERROR querying Property table: " & Err.Description
Else
    view.Execute
    WScript.Echo "Property table query succeeded"
    
    Dim rowCount
    rowCount = 0
    Do
        Set rec = view.Fetch
        If rec Is Nothing Then Exit Do
        rowCount = rowCount + 1
        WScript.Echo "  " & rec.StringData(1) & " = " & rec.StringData(2)
    Loop
    WScript.Echo "Total properties: " & rowCount
End If

' Try to list all tables
On Error Resume Next
Set view = db.OpenView("SELECT * FROM _Tables")
If Err.Number <> 0 Then
    WScript.Echo "ERROR querying _Tables: " & Err.Description
Else
    view.Execute
    WScript.Echo "Tables in database:"
    Do
        Set rec = view.Fetch
        If rec Is Nothing Then Exit Do
        WScript.Echo "  " & rec.StringData(1)
    Loop
End If

WScript.Echo "Done"
