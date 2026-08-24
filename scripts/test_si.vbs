' Test different SummaryInformation approaches
Dim installer, si

Set installer = CreateObject("WindowsInstaller.Installer")

' Approach 1: Empty path (create new)
On Error Resume Next
WScript.Echo "Test 1: Empty path..."
Set si = installer.SummaryInformation("", 20)
If Err.Number <> 0 Then
    WScript.Echo "  Failed: " & Err.Description & " (" & Err.Number & ")"
    Err.Clear
Else
    WScript.Echo "  OK - trying to set Property(1)"
    si.Property(1) = "Test Title"
    If Err.Number <> 0 Then
        WScript.Echo "  Property set failed: " & Err.Description
        Err.Clear
    Else
        WScript.Echo "  Property(1) set OK!"
    End If
End If

' Approach 2: PID 15 (integer) first
WScript.Echo "Test 2: Integer property..."
Set si = installer.SummaryInformation("", 20)
If Err.Number <> 0 Then
    WScript.Echo "  Failed: " & Err.Description
    Err.Clear
Else
    si.Property(15) = 2
    If Err.Number <> 0 Then
        WScript.Echo "  Property(15) failed: " & Err.Description
        Err.Clear
    Else
        WScript.Echo "  Property(15) set OK!"
    End If
End If

' Approach 3: Use the db.SummaryInformation from a freshly created db
Dim fso, db
Set fso = CreateObject("Scripting.FileSystemObject")
If fso.FileExists("C:\temp\test_si.msi") Then fso.DeleteFile "C:\temp\test_si.msi", True

WScript.Echo "Test 3: db.SummaryInformation on fresh db..."
Set db = installer.OpenDatabase("C:\temp\test_si.msi", 3)
Set si = db.SummaryInformation(20)
If Err.Number <> 0 Then
    WScript.Echo "  Failed: " & Err.Description
    Err.Clear
Else
    WScript.Echo "  Got SummaryInfo, trying Property(15) = 2"
    si.Property(15) = 2
    If Err.Number <> 0 Then
        WScript.Echo "  Property(15) failed: " & Err.Description
        Err.Clear
        ' Try reading a property instead
        WScript.Echo "  Trying to READ Property(15)..."
        Dim val
        val = si.Property(15)
        If Err.Number <> 0 Then
            WScript.Echo "  Read also failed: " & Err.Description
            Err.Clear
        Else
            WScript.Echo "  Read OK: " & val
        End If
    Else
        WScript.Echo "  Property(15) set OK!"
    End If
End If

WScript.Echo "Done"
