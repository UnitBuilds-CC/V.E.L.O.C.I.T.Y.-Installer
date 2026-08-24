' Add SummaryInfo to existing MSI using VBScript COM automation
Dim installer, db, si

Set installer = CreateObject("WindowsInstaller.Installer")
Set db = installer.OpenDatabase("C:\temp\com_complete.msi", 1) ' msidbOpenModeTransact

WScript.Echo "Database opened"

' Get SummaryInformation (20 properties max)
Set si = db.SummaryInformation(20)
WScript.Echo "SummaryInformation obtained"

' Set properties
si.Property(1) = "Velocity Test Installation"   ' PID_TITLE
si.Property(2) = "Velocity Test"                 ' PID_SUBJECT  
si.Property(4) = "Velocity Corp"                 ' PID_AUTHOR
si.Property(7) = "Intel;1033"                    ' PID_TEMPLATE
si.Property(9) = CreateObject("Scriptlet.TypeLib").Guid  ' PID_REVNUMBER
si.Property(14) = 0                              ' PID_SECURITY
si.Property(15) = 2                              ' PID_WORDCOUNT
si.Property(19) = ""                             ' PID_PAGECOUNT

WScript.Echo "Properties set"

si.Persist
WScript.Echo "Persisted"

db.Commit
WScript.Echo "Committed"

WScript.Echo "SummaryInfo added successfully"
