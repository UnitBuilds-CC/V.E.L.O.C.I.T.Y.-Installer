' Test: Read and write PID 1 specifically
Dim installer, si, fso, db

Set fso = CreateObject("Scripting.FileSystemObject")
If fso.FileExists("C:\temp\test_pid1.msi") Then fso.DeleteFile "C:\temp\test_pid1.msi", True

Set installer = CreateObject("WindowsInstaller.Installer")
Set db = installer.OpenDatabase("C:\temp\test_pid1.msi", 3)
Set si = db.SummaryInformation(20)

On Error Resume Next

' Try reading PID 1 first
WScript.Echo "Reading PID 1..."
Dim val
val = si.Property(1)
If Err.Number <> 0 Then
    WScript.Echo "  Read failed: " & Err.Description
    Err.Clear
Else
    WScript.Echo "  PID 1 = [" & val & "] (type: " & TypeName(val) & ")"
End If

' Try setting PID 1 to empty string first
WScript.Echo "Setting PID 1 to empty..."
si.Property(1) = ""
If Err.Number <> 0 Then
    WScript.Echo "  Failed: " & Err.Description
    Err.Clear
Else
    WScript.Echo "  OK"
End If

' Now try setting PID 1 to a real value
WScript.Echo "Setting PID 1 to title..."
si.Property(1) = "Test Title"
If Err.Number <> 0 Then
    WScript.Echo "  Failed: " & Err.Description
    Err.Clear
Else
    WScript.Echo "  OK"
End If

' Try CStr conversion
WScript.Echo "Setting PID 1 with CStr..."
si.Property(1) = CStr("Another Title")
If Err.Number <> 0 Then
    WScript.Echo "  Failed: " & Err.Description
    Err.Clear
Else
    WScript.Echo "  OK"
End If

' Check all PIDs
WScript.Echo ""
WScript.Echo "All PIDs:"
Dim i
For i = 1 To 20
    val = si.Property(i)
    If Err.Number = 0 Then
        If Not IsEmpty(val) And val <> "" Then
            WScript.Echo "  PID " & i & " = [" & val & "]"
        End If
    End If
    Err.Clear
Next

db.Commit
