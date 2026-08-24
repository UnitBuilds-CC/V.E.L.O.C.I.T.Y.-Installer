// Create a reference MSI using JScript and Windows Installer COM API
var wi = new ActiveXObject("WindowsInstaller.Installer");
var path = "C:\\temp\\com_reference.msi";

// Delete existing file
var fso = new ActiveXObject("Scripting.FileSystemObject");
if (fso.FileExists(path)) fso.DeleteFile(path);

WScript.Echo("Creating database...");
// OpenDatabase with mode 1 = msiOpenDatabaseModeCreate
var db = wi.OpenDatabase(path, 1);
WScript.Echo("Database created");

// Set SummaryInfo
var si = db.SummaryInformation(1);
si.Property(2) = "Test Product";
si.Property(3) = "Test Product";
si.Property(4) = "Test Company";
si.Property(6) = "Test installation";
si.Property(7) = "x64;1033";
si.Property(9) = "{12345678-1234-4234-8234-123456789012}";
si.Property(14) = 200;
si.Property(15) = 2;
si.Property(18) = "Velocity Installer";
si.Persist();
WScript.Echo("SummaryInfo set");

// Create Property table
var v = db.OpenView("CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL PRIMARY KEY, `Value` CHAR(255) LOCALIZABLE)");
v.Execute();
v.Close();

// Insert properties
var props = [
    ["ProductName", "Test Product"],
    ["ProductVersion", "1.0.0"],
    ["Manufacturer", "Test Company"],
    ["ProductLanguage", "1033"],
    ["ProductCode", "{12345678-1234-4234-8234-123456789012}"],
    ["UpgradeCode", "{ABCDEFAB-ABCD-4ABC-8ABC-ABCDEFABCDEF}"],
    ["ALLUSERS", "1"]
];

for (var i = 0; i < props.length; i++) {
    var sql = "INSERT INTO `Property` (`Property`, `Value`) VALUES ('" + props[i][0] + "', '" + props[i][1] + "')";
    var v2 = db.OpenView(sql);
    v2.Execute();
    v2.Close();
}
WScript.Echo("Properties inserted");

db.Commit();
WScript.Echo("Committed");

var size = fso.GetFile(path).Size;
WScript.Echo("Created: " + path + " (" + size + " bytes)");

// Test with msiexec
WScript.Echo("\nTesting with msiexec...");
var shell = new ActiveXObject("WScript.Shell");
var exec = shell.Exec("msiexec /i \"" + path + "\" /qn /norestart /l*v C:\\temp\\com_ref_install.log");
while (exec.Status == 0) {
    WScript.Sleep(100);
}
WScript.Echo("msiexec exit code: " + exec.ExitCode);

if (exec.ExitCode == 0) {
    WScript.Echo("SUCCESS!");
} else {
    WScript.Echo("FAILED: " + exec.ExitCode);
}
