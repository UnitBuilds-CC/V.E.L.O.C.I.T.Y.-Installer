// Create reference MSI using JScript and Windows Installer COM API
var installer = new ActiveXObject("WindowsInstaller.Installer");
var fso = new ActiveXObject("Scripting.FileSystemObject");

var msiPath = "C:\\temp\\ref_com.msi";

// Delete existing file if present
if (fso.FileExists(msiPath)) fso.DeleteFile(msiPath, true);

// Create database - mode 6 = msiOpenDatabaseModeCreateDirect
var db;
try {
    db = installer.OpenDatabase(msiPath, 6);
    WScript.Echo("Created with mode 6");
} catch(e) {
    WScript.Echo("Mode 6 failed: " + e.message);
    
    // Try mode 1 = msiOpenDatabaseModeTransact
    try {
        // First create an empty file
        var f = fso.CreateTextFile(msiPath, true);
        f.Close();
        fso.DeleteFile(msiPath, true);
        
        db = installer.OpenDatabase(msiPath, 6);
        WScript.Echo("Created with mode 6 (retry)");
    } catch(e2) {
        WScript.Echo("Retry also failed: " + e2.message);
        WScript.Quit(1);
    }
}

// Set SummaryInfo
var si = db.SummaryInformation(1);
si.Property(1) = "1252";          // Codepage
si.Property(2) = "Reference MSI"; // Title
si.Property(4) = "Test Author";   // Author
si.Property(7) = "x64;1033";      // Template
si.Property(9) = "{12345678-1234-1234-1234-123456789012}"; // Revision
si.Property(14) = "405";          // Security
si.Property(15) = 2;              // WordCount
si.Property(18) = "COM Creator";  // Creating App
si.Flush();
WScript.Echo("SummaryInfo set");

// Create Property table
db.Execute("CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(255) NULL LOCALIZABLE PRIMARY KEY `Property`)");
WScript.Echo("Property table created");

// Insert properties
function InsertProp(name, value) {
    var rec = installer.CreateRecord(2);
    rec.StringData(1) = name;
    rec.StringData(2) = value;
    db.Execute("INSERT INTO `Property` VALUES (?, ?)", rec);
}

InsertProp("ProductName", "Reference MSI");
InsertProp("ProductCode", "{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}");
InsertProp("ProductVersion", "1.0.0");
InsertProp("Manufacturer", "TestCorp");
InsertProp("UpgradeCode", "{BBBBBBBB-CCCC-DDDD-EEEE-FFFFFFFFFFFF}");
InsertProp("ProductLanguage", "1033");
InsertProp("ALLUSERS", "1");
WScript.Echo("Property table: 7 rows");

db.Commit();
WScript.Echo("Database committed");

// Check file size
var fileSize = fso.GetFile(msiPath).Size;
WScript.Echo("MSI size: " + fileSize + " bytes");

// Test with msiexec
WScript.Echo("\n=== Testing with msiexec ===");
var shell = new ActiveXObject("WScript.Shell");
var ret = shell.Run("msiexec /i " + msiPath + " /qn /norestart /l*v C:\\temp\\ref_com.log", 0, true);
WScript.Echo("msiexec exit code: " + ret);

if (ret == 0) {
    WScript.Echo("*** SUCCESS! ***");
} else {
    WScript.Echo("Failed with code " + ret);
    // Read last 30 lines of log
    if (fso.FileExists("C:\\temp\\ref_com.log")) {
        var log = fso.OpenTextFile("C:\\temp\\ref_com.log", 1);
        var lines = [];
        while (!log.AtEndOfStream) {
            lines.push(log.ReadLine());
        }
        log.Close();
        var start = Math.max(0, lines.length - 30);
        for (var i = start; i < lines.length; i++) {
            WScript.Echo(lines[i]);
        }
    }
}
