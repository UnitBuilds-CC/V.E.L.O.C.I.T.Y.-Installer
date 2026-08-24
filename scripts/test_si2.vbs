' Test SummaryInfo string properties on fresh database
Dim installer, si, fso, db

Set fso = CreateObject("Scripting.FileSystemObject")
Set installer = CreateObject("WindowsInstaller.Installer")

If fso.FileExists("C:\temp\test_si2.msi") Then fso.DeleteFile "C:\temp\test_si2.msi", True

Set db = installer.OpenDatabase("C:\temp\test_si2.msi", 3)
Set si = db.SummaryInformation(20)

On Error Resume Next

' Test integer properties
WScript.Echo "Testing integer properties..."
si.Property(14) = 0
WScript.Echo "  PID 14 (Security): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

si.Property(15) = 2
WScript.Echo "  PID 15 (WordCount): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

' Test string properties
WScript.Echo "Testing string properties..."
si.Property(1) = "Test Title"
WScript.Echo "  PID 1 (Title): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

si.Property(2) = "Test Subject"
WScript.Echo "  PID 2 (Subject): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

si.Property(4) = "Test Author"
WScript.Echo "  PID 4 (Author): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

si.Property(7) = "Intel;1033"
WScript.Echo "  PID 7 (Template): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

si.Property(9) = "{12345678-1234-1234-1234-123456789012}"
WScript.Echo "  PID 9 (RevNumber): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

si.Property(19) = ""
WScript.Echo "  PID 19 (PageCount): " & IIf(Err.Number = 0, "OK", "FAIL: " & Err.Description)
Err.Clear

' Persist and commit
si.Persist
If Err.Number <> 0 Then
    WScript.Echo "Persist failed: " & Err.Description
    Err.Clear
Else
    WScript.Echo "Persist OK"
End If

db.Commit
WScript.Echo "Commit OK"

Dim size
size = fso.GetFile("C:\temp\test_si2.msi").Size
WScript.Echo "File size: " & size & " bytes"

Function IIf(cond, t, f)
    If cond Then IIf = t Else IIf = f
End Function
