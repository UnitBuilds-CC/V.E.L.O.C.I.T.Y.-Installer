' Create MSI database with Property table via COM (no SummaryInfo)
' SummaryInfo will be added by Rust msi crate
Dim installer, db, view, fso

Set fso = CreateObject("Scripting.FileSystemObject")
If fso.FileExists("C:\temp\com_base.msi") Then fso.DeleteFile "C:\temp\com_base.msi", True

Set installer = CreateObject("WindowsInstaller.Installer")
Set db = installer.OpenDatabase("C:\temp\com_base.msi", 3)

' Create Property table
Set view = db.OpenView("CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL LOCALIZABLE, `Value` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY `Property`)")
view.Execute : view.Close

' Insert rows
Set view = db.OpenView("INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductName', 'Velocity Test')")
view.Execute : view.Close
Set view = db.OpenView("INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductVersion', '1.0.0')")
view.Execute : view.Close
Set view = db.OpenView("INSERT INTO `Property` (`Property`, `Value`) VALUES ('Manufacturer', 'Velocity Corp')")
view.Execute : view.Close

db.Commit
WScript.Echo "COM database created: C:\temp\com_base.msi"
