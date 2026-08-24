' Test: Add SummaryInfo using installer.SummaryInformation(path, count)
Dim installer, si, db, fso

Set fso = CreateObject("Scripting.FileSystemObject")
Set installer = CreateObject("WindowsInstaller.Installer")

' Check if com_full.msi exists from Phase 1
If Not fso.FileExists("C:\temp\com_full.msi") Then
    WScript.Echo "ERROR: C:\temp\com_full.msi not found. Run Phase 1 first."
    WScript.Quit 1
End If

WScript.Echo "Opening SummaryInformation from file path..."

' Method: Create SummaryInformation from file path (not from db object)
Set si = installer.SummaryInformation("C:\temp\com_full.msi", 20)
WScript.Echo "SummaryInformation object created"

' Try setting property
On Error Resume Next
si.Property(1) = "Velocity Test Installation"
If Err.Number <> 0 Then
    WScript.Echo "Property(1) setter failed: " & Err.Description
    Err.Clear
    
    ' Try SetProperty method
    si.SetProperty 1, "Velocity Test Installation"
    If Err.Number <> 0 Then
        WScript.Echo "SetProperty also failed: " & Err.Description
        Err.Clear
        
        ' Try using the Installer.CreateSummaryInformation approach
        WScript.Echo "Trying CreateSummaryInformation approach..."
        Set si = installer.CreateSummaryInformation("C:\temp\com_full.msi", 20)
        If Err.Number <> 0 Then
            WScript.Echo "CreateSummaryInformation failed: " & Err.Description
            WScript.Quit 1
        End If
        si.Property(1) = "Velocity Test Installation"
        If Err.Number <> 0 Then
            WScript.Echo "Still failed: " & Err.Description
            WScript.Quit 1
        End If
    End If
End If
On Error GoTo 0

WScript.Echo "Title set successfully!"

si.Property(2) = "Velocity Test"
si.Property(4) = "Velocity Corp"
si.Property(7) = "Intel;1033"
si.Property(9) = CreateObject("Scriptlet.TypeLib").Guid
si.Property(14) = 0
si.Property(15) = 2
WScript.Echo "All properties set"

si.Persist
WScript.Echo "Persisted"

WScript.Echo "SummaryInfo added successfully!"
