# Use Windows Installer COM API to diagnose our MSI using ReadDirect mode
$ErrorActionPreference = "Continue"

$msiPath = "C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\just_prop.msi"

Write-Host "=== Opening MSI with ReadDirect mode ==="
Write-Host "File size: $((Get-Item $msiPath).Length) bytes"

$inst = New-Object -ComObject WindowsInstaller.Installer

# Open in ReadDirect mode (3) - bypasses validation
$db = $inst.OpenDatabase($msiPath, 3)
Write-Host "Database opened successfully"

# Enumerate tables via _Tables
Write-Host "`n=== Tables in database ==="
try {
    $view = $db.OpenView("SELECT * FROM _Tables")
    $view.Execute()
    $rec = $view.Fetch()
    while ($rec -ne $null) {
        $tableName = $rec.StringData(1)
        Write-Host "  Table: $tableName"
        $rec = $view.Fetch()
    }
    $view.Close()
} catch {
    Write-Host "ERROR: $_"
}

# Read _Columns
Write-Host "`n=== _Columns entries ==="
try {
    $view = $db.OpenView("SELECT * FROM _Columns")
    $view.Execute()
    $rec = $view.Fetch()
    while ($rec -ne $null) {
        $tbl = $rec.StringData(1)
        $num = $rec.IntegerData(2)
        $col = $rec.StringData(3)
        $typ = $rec.IntegerData(4)
        Write-Host "  $tbl.$num $col type=0x$($typ.ToString('X4'))"
        $rec = $view.Fetch()
    }
    $view.Close()
} catch {
    Write-Host "ERROR: $_"
}

# Read Property table
Write-Host "`n=== Property table ==="
try {
    $view = $db.OpenView("SELECT * FROM Property")
    $view.Execute()
    $rec = $view.Fetch()
    while ($rec -ne $null) {
        $prop = $rec.StringData(1)
        $val = $rec.StringData(2)
        Write-Host "  $prop = $val"
        $rec = $view.Fetch()
    }
    $view.Close()
} catch {
    Write-Host "ERROR: $_"
}

# Read SummaryInformation
Write-Host "`n=== SummaryInformation ==="
try {
    $sum = $db.SummaryInformation(0)
    $pids = @(1,2,3,4,5,6,7,8,9,12,13,14,15,18,19)
    foreach ($p in $pids) {
        try {
            $val = $sum.Property($p)
            if ($val -ne $null -and "$val" -ne "") {
                Write-Host "  PID $p = $val"
            }
        } catch {}
    }
} catch {
    Write-Host "ERROR reading SummaryInfo: $_"
}

# Read _Validation
Write-Host "`n=== _Validation ==="
try {
    $view = $db.OpenView("SELECT * FROM _Validation")
    $view.Execute()
    $count = 0
    $rec = $view.Fetch()
    while ($rec -ne $null) {
        $count++
        $tbl = $rec.StringData(1)
        $col = $rec.StringData(2)
        $nul = $rec.StringData(3)
        $cat = $rec.StringData(8)
        Write-Host "  $tbl.$col nullable=$nul category=$cat"
        $rec = $view.Fetch()
    }
    Write-Host "  Total rows: $count"
    $view.Close()
} catch {
    Write-Host "  _Validation error: $_"
}

# Test ReadOnly mode
Write-Host "`n=== Testing ReadOnly mode ==="
try {
    $db2 = $inst.OpenDatabase($msiPath, 0)
    Write-Host "  ReadOnly mode: SUCCESS"
} catch {
    Write-Host "  ReadOnly mode: FAILED"
}
