// Create reference MSI using JScript and Windows Installer COM API
var installer = new ActiveXObject("WindowsInstaller.Installer");
var fso = new ActiveXObject("Scripting.FileSystemObject");

// Clean up
if (fso.FileExists("C:\\temp\\ref_jscript.msi")) fso.DeleteFile("C:\\temp\\ref_jscript.msi");

// Create database - try different approaches
var db;
try {
    // Mode 6 = msiOpenDatabaseModeCreateDirect
    db = installer.OpenDatabase("C:\\temp\\ref_jscript.msi", 6);
    WScript.Echo("Created with mode 6");
} catch(e) {
    WScript.Echo("Mode 6 failed: " + e.message);
    
    // Try creating empty file first
    var f = fso.CreateTextFile("C:\\temp\\ref_jscript.msi", true);
    f.Close();
    
    try {
        db = installer.OpenDatabase("C:\\temp\\ref_jscript.msi", 6);
        WScript.Echo("Created with mode 6 after file creation");
    } catch(e2) {
        WScript.Echo("Mode 6 still failed: " + e2.message);
        
        // Try mode 2 (direct read/write) on empty file
        try {
            db = installer.OpenDatabase("C:\\temp\\ref_jscript.msi", 2);
            WScript.Echo("Created with mode 2");
        } catch(e3) {
            WScript.Echo("Mode 2 also failed: " + e3.message);
            WScript.Quit(1);
        }
    }
}

// Set SummaryInfo
var si = db.SummaryInformation(1);
si.Property(1) = "1252";
si.Property(2) = "Reference MSI";
si.Property(4) = "Test Author";
si.Property(7) = "x64;1033";
si.Property(9) = "{12345678-1234-1234-1234-123456789012}";
si.Property(14) = "405";
si.Property(15) = 2;
si.Property(18) = "JScript Creator";
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
var fileSize = fso.GetFile("C:\\temp\\ref_jscript.msi").Size;
WScript.Echo("MSI size: " + fileSize + " bytes");

// Test with msiexec
WScript.Echo("\n=== Testing with msiexec ===");
var shell = new ActiveXObject("WScript.Shell");
var ret = shell.Run("msiexec /i C:\\temp\\ref_jscript.msi /qn /norestart /l*v C:\\temp\\ref_jscript.log", 0, true);
WScript.Echo("msiexec exit code: " + ret);

if (ret == 0) {
    WScript.Echo("*** SUCCESS! ***");
} else {
    WScript.Echo("Failed with code " + ret);
    // Read log
    if (fso.FileExists("C:\\temp\\ref_jscript.log")) {
        var log = fso.OpenTextFile("C:\\temp\\ref_jscript.log", 1);
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
