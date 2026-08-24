# Test: Open MSI with ReadDirect, save it, and see if the saved version works
$ErrorActionPreference = "Continue"

$srcPath = "C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\just_prop.msi"
$dstPath = "C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\resaved.msi"

if (Test-Path $dstPath) { Remove-Item $dstPath -Force }

$inst = New-Object -ComObject WindowsInstaller.Installer

# Open in ReadDirect mode
$db = $inst.OpenDatabase($srcPath, 3)
Write-Host "Opened source with ReadDirect mode"

# Try to commit/save to a new file
# First, let's see if we can create a new database from this one
# OpenDatabase with mode 2 = create
try {
    # Copy the file
    Copy-Item $srcPath $dstPath -Force
    Write-Host "Copied file"
    
    # Open the copy in transact mode (mode 1 = read/write)
    $db2 = $inst.OpenDatabase($dstPath, 1)
    Write-Host "Opened copy in transact mode"
    
    # Try to read SummaryInfo from the copy
    $sum = $db2.SummaryInformation(0)
    Write-Host "Title: $($sum.Property(2))"
    Write-Host "RevNumber: $($sum.Property(9))"
    Write-Host "Template: $($sum.Property(7))"
    Write-Host "WordCount: $($sum.Property(15))"
    
    # Set a property to force a write
    $sum.Property(6) = "Resaved by COM API"
    $sum.Persist()
    $db2.Commit()
    Write-Host "Saved changes"
    
    # Now try to open the resaved file in ReadOnly mode
    try {
        $db3 = $inst.OpenDatabase($dstPath, 0)
        Write-Host "RESAVED: ReadOnly mode SUCCESS!"
        
        # Read tables
        $view = $db3.OpenView("SELECT * FROM _Tables")
        $view.Execute()
        $rec = $view.Fetch()
        while ($rec -ne $null) {
            Write-Host "  Table: $($rec.StringData(1))"
            $rec = $view.Fetch()
        }
        $view.Close()
    } catch {
        Write-Host "RESAVED: ReadOnly mode FAILED"
    }
    
} catch {
    Write-Host "Error: $_"
}

# Also test: create a completely new MSI using COM API and compare
$newPath = "C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\com_created.msi"
if (Test-Path $newPath) { Remove-Item $newPath -Force }

Write-Host "`n=== Creating new MSI via COM API ==="
try {
    # Mode 2 = create new database  
    $newDb = $inst.OpenDatabase($newPath, 2)
    Write-Host "Created new database"
    
    # Create Property table
    $v = $newDb.OpenView("CREATE TABLE ``Property`` (``Property`` CHAR(72) NOT NULL PRIMARY KEY ``Property``, ``Value`` CHAR(255) NULL)")
    $v.Execute()
    $v.Close()
    Write-Host "Created Property table"
    
    # Insert a row
    $v = $newDb.OpenView("INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductName', 'COM Test')")
    $v.Execute()
    $v.Close()
    
    # Set summary info
    $sum = $newDb.SummaryInformation(0)
    $sum.Property(1) = 1252  # Codepage
    $sum.Property(2) = "COM Created"
    $sum.Property(9) = "{11111111-2222-3333-4444-555555555555}"
    $sum.Property(14) = "x64;1033"
    $sum.Persist()
    
    $newDb.Commit()
    Write-Host "Committed. Size: $((Get-Item $newPath).Length) bytes"
    
    # Try to open it ReadOnly
    try {
        $testDb = $inst.OpenDatabase($newPath, 0)
        Write-Host "COM created MSI: ReadOnly SUCCESS"
    } catch {
        Write-Host "COM created MSI: ReadOnly FAILED"
    }
} catch {
    Write-Host "COM create error: $_"
}
