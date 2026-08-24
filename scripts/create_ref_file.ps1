# Create a reference MSI with a File table using Windows Installer COM API
# Then read the _Validation entries to see what Windows Installer expects

$wi = New-Object -ComObject WindowsInstaller.Installer
$bt = [char]96  # backtick character for MSI SQL

# Create database (mode 1 = msiOpenDatabaseModeCreateDirect)
$database = $wi.OpenDatabase("C:\temp\com_ref.msi", 1)

# Create Property table
$sql = "CREATE TABLE " + $bt + "Property" + $bt + " (" + $bt + "Property" + $bt + " CHAR(72) NOT NULL, " + $bt + "Value" + $bt + " CHAR(255) NULL LOCALIZABLE PRIMARY KEY " + $bt + "Property" + $bt + ")"
Write-Host "Creating Property table..."
$view = $database.OpenView($sql)
$view.Execute()
$view.Close()

# Insert properties
$props = @(
    @("ProductName", "COM Reference"),
    @("ProductVersion", "1.0.0"),
    @("Manufacturer", "Test"),
    @("ProductCode", [System.Guid]::NewGuid().ToString("B")),
    @("UpgradeCode", [System.Guid]::NewGuid().ToString("B")),
    @("ProductLanguage", "1033")
)

foreach ($p in $props) {
    $sql = "INSERT INTO " + $bt + "Property" + $bt + " (" + $bt + "Property" + $bt + ", " + $bt + "Value" + $bt + ") VALUES ('" + $p[0] + "', '" + $p[1] + "')"
    $view = $database.OpenView($sql)
    $view.Execute()
    $view.Close()
}
Write-Host "Properties inserted."

# Create File table
$sql = "CREATE TABLE " + $bt + "File" + $bt + " (" + $bt + "File_" + $bt + " CHAR(72) NOT NULL, " + $bt + "Component_" + $bt + " CHAR(72), " + $bt + "FileName" + $bt + " CHAR(255) LOCALIZABLE, " + $bt + "FileSize" + $bt + " LONG, " + $bt + "Attributes" + $bt + " SHORT, " + $bt + "Sequence" + $bt + " SHORT PRIMARY KEY " + $bt + "File_" + $bt + ")"
Write-Host "Creating File table..."
$view = $database.OpenView($sql)
$view.Execute()
$view.Close()

# Insert a File row
$sql = "INSERT INTO " + $bt + "File" + $bt + " (" + $bt + "File_" + $bt + ", " + $bt + "Component_" + $bt + ", " + $bt + "FileName" + $bt + ", " + $bt + "FileSize" + $bt + ", " + $bt + "Attributes" + $bt + ", " + $bt + "Sequence" + $bt + ") VALUES ('MainFile', 'MainComp', 'testfile.txt', 23, 0, 1)"
Write-Host "Inserting File row..."
$view = $database.OpenView($sql)
$view.Execute()
$view.Close()

# Commit
$database.Commit()
Write-Host "Database committed."

# Reopen for reading
$database = $wi.OpenDatabase("C:\temp\com_ref.msi", 0)

# Read _Validation entries for File table
Write-Host "`n=== _Validation entries for File table ==="
$sql = "SELECT * FROM " + $bt + "_Validation" + $bt + " WHERE " + $bt + "Table" + $bt + " = 'File'"
$view = $database.OpenView($sql)
$view.Execute()

$record = $view.Fetch()
$row = 0
while ($record -ne $null) {
    $row++
    $vals = @()
    for ($i = 1; $i -le 10; $i++) {
        try {
            $v = $record.StringData($i)
            if ($v -eq $null) { $v = "NULL" }
        } catch {
            try {
                $v = $record.IntegerData($i)
            } catch {
                $v = "NULL"
            }
        }
        $vals += $v
    }
    Write-Host ("Row {0}: Table={1} Col={2} Nullable={3} Min={4} Max={5} KeyTable={6} KeyCol={7} Category={8} Set={9} Desc={10}" -f $row, $vals[0], $vals[1], $vals[2], $vals[3], $vals[4], $vals[5], $vals[6], $vals[7], $vals[8], $vals[9])
    $record = $view.Fetch()
}
Write-Host "Total _Validation rows for File: $row"
$view.Close()

# Read _Columns entries for File table
Write-Host "`n=== _Columns entries for File table ==="
$sql = "SELECT * FROM " + $bt + "_Columns" + $bt + " WHERE " + $bt + "Table" + $bt + " = 'File'"
$view = $database.OpenView($sql)
$view.Execute()

$record = $view.Fetch()
$row = 0
while ($record -ne $null) {
    $row++
    $tbl = $record.StringData(1)
    $num = $record.IntegerData(2)
    $col = $record.StringData(3)
    $typ = $record.IntegerData(4)
    Write-Host ("Row {0}: Table={1} Number={2} Column={3} Type=0x{4:X4} ({4})" -f $row, $tbl, $num, $col, $typ)
    $record = $view.Fetch()
}
Write-Host "Total _Columns rows for File: $row"
$view.Close()

# Test with msiexec
Write-Host "`n=== Testing with msiexec ==="
$proc = Start-Process -FilePath "msiexec.exe" -ArgumentList "/i", "C:\temp\com_ref.msi", "/qn", "/l*v", "C:\temp\com_ref.log" -Wait -PassThru
Write-Host "msiexec exit code: $($proc.ExitCode)"

if ($proc.ExitCode -ne 0) {
    Write-Host "`nError lines from log:"
    Get-Content "C:\temp\com_ref.log" | Where-Object { $_ -match "return value 3|Note:" } | ForEach-Object { Write-Host "  $_" }
}

$database = $null
$wi = $null
Write-Host "`n=== DONE ==="
