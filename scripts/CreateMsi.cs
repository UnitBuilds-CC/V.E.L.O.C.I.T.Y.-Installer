using System;
using System.Runtime.InteropServices;

class CreateMsi
{
    [DllImport("msi.dll", CharSet = CharSet.Unicode)]
    static extern int MsiOpenDatabase(string dbPath, IntPtr persist);
    
    [DllImport("msi.dll", CharSet = CharSet.Unicode)]
    static extern int MsiCloseHandle(int hDB);

    // Use COM interop instead
    static void Main()
    {
        string path = @"C:\temp\com_ref.msi";
        if (System.IO.File.Exists(path)) System.IO.File.Delete(path);

        try
        {
            // Create WindowsInstaller COM object
            Type installerType = Type.GetTypeFromProgID("WindowsInstaller.Installer");
            dynamic installer = Activator.CreateInstance(installerType);
            
            // OpenDatabase with create mode (msoOpenDatabaseModeCreate=1 | msoOpenDatabaseModeTransact=2 = 3)
            dynamic db = installer.GetType().InvokeMember("OpenDatabase",
                System.Reflection.BindingFlags.InvokeMethod, null, installer,
                new object[] { path, 3 });
            Console.WriteLine("Database created");

            // Create Property table
            dynamic view = db.GetType().InvokeMember("OpenView",
                System.Reflection.BindingFlags.InvokeMethod, null, db,
                new object[] { "CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(255))" });
            view.GetType().InvokeMember("Execute",
                System.Reflection.BindingFlags.InvokeMethod, null, view, null);
            Console.WriteLine("Property table created");

            // Insert properties using Record
            string[] names = { "ProductName", "ProductCode", "ProductVersion", "Manufacturer", "UpgradeCode", "ALLUSERS", "ProductLanguage" };
            string[] values = { "Test Product", "{A0A0A0A0-B1B1-42C2-83D3-E4E4E4E4E4E4}", "1.0.0", "Test Mfg", "{B1B1B1B1-C2C2-43D3-94E4-F5F5F5F5F5F5}", "1", "1033" };

            for (int i = 0; i < names.Length; i++)
            {
                dynamic rec = installer.GetType().InvokeMember("CreateRecord",
                    System.Reflection.BindingFlags.InvokeMethod, null, installer,
                    new object[] { 2 });
                rec.GetType().InvokeMember("set_StringData",
                    System.Reflection.BindingFlags.InvokeMethod, null, rec,
                    new object[] { 1, names[i] });
                rec.GetType().InvokeMember("set_StringData",
                    System.Reflection.BindingFlags.InvokeMethod, null, rec,
                    new object[] { 2, values[i] });
                    
                dynamic insView = db.GetType().InvokeMember("OpenView",
                    System.Reflection.BindingFlags.InvokeMethod, null, db,
                    new object[] { "INSERT INTO `Property` (`Property`, `Value`) VALUES (?, ?)" });
                insView.GetType().InvokeMember("Execute",
                    System.Reflection.BindingFlags.InvokeMethod, null, insView,
                    new object[] { rec });
            }
            Console.WriteLine("Properties inserted");

            db.GetType().InvokeMember("Commit",
                System.Reflection.BindingFlags.InvokeMethod, null, db, null);
            Console.WriteLine("Database committed");

            // Set SummaryInfo
            dynamic si = db.GetType().InvokeMember("SummaryInformation",
                System.Reflection.BindingFlags.InvokeMethod, null, db,
                new object[] { 0 });
            
            si.GetType().InvokeMember("set_Property",
                System.Reflection.BindingFlags.InvokeMethod, null, si,
                new object[] { 1, 1252 });
            si.GetType().InvokeMember("set_Property",
                System.Reflection.BindingFlags.InvokeMethod, null, si,
                new object[] { 2, "Installation Database" });
            si.GetType().InvokeMember("set_Property",
                System.Reflection.BindingFlags.InvokeMethod, null, si,
                new object[] { 4, "Test Author" });
            si.GetType().InvokeMember("set_Property",
                System.Reflection.BindingFlags.InvokeMethod, null, si,
                new object[] { 7, "x64;1033" });
            si.GetType().InvokeMember("set_Property",
                System.Reflection.BindingFlags.InvokeMethod, null, si,
                new object[] { 9, "{A0A0A0A0-B1B1-42C2-83D3-E4E4E4E4E4E4}" });
            si.GetType().InvokeMember("set_Property",
                System.Reflection.BindingFlags.InvokeMethod, null, si,
                new object[] { 15, 2 });
            si.GetType().InvokeMember("set_Property",
                System.Reflection.BindingFlags.InvokeMethod, null, si,
                new object[] { 18, "Velocity Installer" });
            si.GetType().InvokeMember("Persist",
                System.Reflection.BindingFlags.InvokeMethod, null, si, null);
            db.GetType().InvokeMember("Commit",
                System.Reflection.BindingFlags.InvokeMethod, null, db, null);
            Console.WriteLine("SummaryInfo committed");

            var fi = new System.IO.FileInfo(path);
            Console.WriteLine("COM MSI created: " + fi.Length + " bytes");
        }
        catch (Exception ex)
        {
            Console.WriteLine("ERROR: " + ex.Message);
            if (ex.InnerException != null)
                Console.WriteLine("INNER: " + ex.InnerException.Message);
            Console.WriteLine("STACK: " + ex.StackTrace);
            Environment.Exit(1);
        }
    }
}
