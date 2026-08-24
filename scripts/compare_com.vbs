Option Explicit

Dim wi, fso
Set fso = CreateObject("Scripting.FileSystemObject")
Set wi = CreateObject("WindowsInstaller.Installer")

TestFile "Real MSI", "C:\WINDOWS\Installer\10d16cbb.msi"
TestFile "msi crate (diag)", "C:\temp\diag_msi.msi"
TestFile "msi crate (utf8)", "C:\temp\msi_utf8.msi"
TestFile "msi crate (1252)", "C:\temp\msi_1252.msi"
TestFile "complete msi", "C:\temp\complete_msi.msi"

Sub TestFile(label, path)
    If Not fso.FileExists(path) Then
        WScript.Echo label & ": FILE NOT FOUND"
        Exit Sub
    End If
    
    WScript.Echo label & " (" & fso.GetFile(path).Size & " bytes): "
    
    On Error Resume Next
    Dim db
    Set db = wi.OpenDatabase(path, 0)
    If Err.Number <> 0 Then
        WScript.Echo "  OPEN FAILED: " & Err.Description & " (0x" & Hex(Err.Number) & ")"
    Else
        WScript.Echo "  OPEN SUCCESS"
        Dim si
        Set si = db.SummaryInformation(0)
        On Error Resume Next
        WScript.Echo "  Title: " & si.Property(2)
        WScript.Echo "  Codepage: " & si.Property(1)
        WScript.Echo "  Template: " & si.Property(7)
    End If
    On Error GoTo 0
End Sub
