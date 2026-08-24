Set wi = CreateObject("WindowsInstaller.Installer")
Set db = wi.CreateDatabase("ref_com.msi", 1)

' Create Directory table
Set view = db.OpenView("CREATE TABLE `Directory` (`Directory` CHAR(72) NOT NULL, `Directory_Parent` CHAR(72) NULL, `DefaultDir` CHAR(255) NOT NULL PRIMARY KEY)")
view.Execute

' Insert TARGETDIR row
Set rec = wi.CreateRecord(3)
rec.SetString 1, "TARGETDIR"
rec.SetString 2, ""
rec.SetString 3, "SourceDir"
Set view = db.OpenView("SELECT * FROM Directory")
view.Execute
view.Modify 2, rec

' Create Property table
Set view = db.OpenView("CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL PRIMARY KEY, `Value` CHAR(255) NULL)")
view.Execute
Set rec = wi.CreateRecord(2)
rec.SetString 1, "ProductName"
rec.SetString 2, "RefTest"
Set view = db.OpenView("SELECT * FROM Property")
view.Execute
view.Modify 2, rec

Set rec = wi.CreateRecord(2)
rec.SetString 1, "ProductVersion"
rec.SetString 2, "1.0.0"
view.Modify 2, rec

Set rec = wi.CreateRecord(2)
rec.SetString 1, "ProductCode"
rec.SetString 2, "{CD2B1AE6-B087-747A-97CB-7C816A89D5B6}"
view.Modify 2, rec

Set rec = wi.CreateRecord(2)
rec.SetString 1, "ProductLanguage"
rec.SetString 2, "1033"
view.Modify 2, rec

db.Commit
WScript.Echo "Created ref_com.msi"
