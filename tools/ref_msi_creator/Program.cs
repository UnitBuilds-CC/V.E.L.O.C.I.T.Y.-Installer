using System;
using System.IO;
using System.Diagnostics;

class Program
{
    static void Main()
    {
        string msiPath = @"C:\temp\ref_com.msi";
        if (File.Exists(msiPath)) File.Delete(msiPath);
        Directory.CreateDirectory(@"C:\temp");

        Console.WriteLine("Creating reference MSI via Windows Installer COM API...");

        Type? installerType = Type.GetTypeFromProgID("WindowsInstaller.Installer");
        if (installerType == null)
        {
            Console.WriteLine("ERROR: WindowsInstaller.Installer COM object not found");
            return;
        }
        dynamic installer = Activator.CreateInstance(installerType)!;

        dynamic db;
        try
        {
            db = installer.OpenDatabase(msiPath, 6);
            Console.WriteLine("Database created with mode 6");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Mode 6 failed: {ex.Message}");
            return;
        }

        // Set Summary Information
        dynamic si = db.SummaryInformation(1);
        si.set_Property(1, "1252");
        si.set_Property(2, "Reference MSI");
        si.set_Property(4, "Test Author");
        si.set_Property(7, "x64;1033");
        si.set_Property(9, "{12345678-1234-1234-1234-123456789012}");
        si.set_Property(14, "405");
        si.set_Property(15, "2");
        si.set_Property(18, "COM Reference Creator");
        si.Flush();
        Console.WriteLine("SummaryInfo set");

        // Create Property table
        db.Execute("CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(255) NULL LOCALIZABLE PRIMARY KEY `Property`)");
        Console.WriteLine("Property table created");

        InsertProp(db, installer, "ProductName", "Reference MSI");
        InsertProp(db, installer, "ProductCode", "{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}");
        InsertProp(db, installer, "ProductVersion", "1.0.0");
        InsertProp(db, installer, "Manufacturer", "TestCorp");
        InsertProp(db, installer, "UpgradeCode", "{BBBBBBBB-CCCC-DDDD-EEEE-FFFFFFFFFFFF}");
        InsertProp(db, installer, "ProductLanguage", "1033");
        InsertProp(db, installer, "ALLUSERS", "1");
        Console.WriteLine("Property table: 7 rows");

        db.Commit();
        Console.WriteLine($"Database committed. Size: {new FileInfo(msiPath).Length} bytes");

        // Test with msiexec
        Console.WriteLine("\n=== Testing with msiexec ===");
        var psi = new ProcessStartInfo("msiexec.exe",
            $"/i \"{msiPath}\" /qn /norestart /l*v C:\\temp\\ref_com.log")
        { UseShellExecute = false };
        var proc = Process.Start(psi);
        proc!.WaitForExit();
        Console.WriteLine($"msiexec exit code: {proc.ExitCode}");

        if (proc.ExitCode == 0)
            Console.WriteLine("*** SUCCESS! ***");
        else
        {
            Console.WriteLine($"Failed with code {proc.ExitCode}");
            if (File.Exists(@"C:\temp\ref_com.log"))
            {
                var lines = File.ReadAllLines(@"C:\temp\ref_com.log");
                for (int i = Math.Max(0, lines.Length - 30); i < lines.Length; i++)
                    Console.WriteLine(lines[i]);
            }
        }
    }

    static void InsertProp(dynamic db, dynamic installer, string name, string value)
    {
        dynamic rec = installer.CreateRecord(2);
        rec.set_StringData(1, name);
        rec.set_StringData(2, value);
        db.Execute("INSERT INTO `Property` VALUES (?, ?)", rec);
    }
}
