$msiPath = "C:\temp\ref_good.msi"
$wi = New-Object -ComObject WindowsInstaller.Installer

try {
    Write-Host "Opening MSI: $msiPath"
    $db = $wi.OpenDatabase($msiPath, 0)
    Write-Host "Opened OK"
    
    # Read Property table
    Write-Host "`n=== Property Table ==="
    $view = $db.OpenView("SELECT * FROM Property")
    $view.Execute()
    while ($rec = $view.Fetch()) {
        $prop = $rec.StringData(1)
        $val = $rec.StringData(2)
        Write-Host "  $prop = $val"
    }
    $view.Close()
    
    # Read SummaryInfo
    Write-Host "`n=== SummaryInfo ==="
    $sumInfo = $db.SummaryInformation(0)
    Write-Host "  Title: $($sumInfo.Property(2))"
    Write-Host "  Subject: $($sumInfo.Property(3))"
    Write-Host "  Author: $($sumInfo.Property(4))"
    Write-Host "  Comments: $($sumInfo.Property(6))"
    Write-Host "  Template: $($sumInfo.Property(7))"
    Write-Host "  LastAuthor: $($sumInfo.Property(8))"
    Write-Host "  RevNumber: $($sumInfo.Property(9))"
    Write-Host "  CreateDtm: $($sumInfo.Property(12))"
    Write-Host "  LastSaveDtm: $($sumInfo.Property(13))"
    Write-Host "  PageCount: $($sumInfo.Property(14))"
    Write-Host "  WordCount: $($sumInfo.Property(15))"
    
} catch {
    Write-Host "ERROR: $($_.Exception.Message)"
    Write-Host "HRESULT: $($_.Exception.HResult)"
}

# Also try the velocity-msi output
Write-Host "`n`n=== Testing velocity-msi output (cfb repacked) ==="
try {
    $db2 = $wi.OpenDatabase("C:\temp\ref_velocity_cfb.msi", 0)
    Write-Host "Opened OK"
    
    $view2 = $db2.OpenView("SELECT * FROM Property")
    $view2.Execute()
    while ($rec = $view2.Fetch()) {
        $prop = $rec.StringData(1)
        $val = $rec.StringData(2)
        Write-Host "  $prop = $val"
    }
    $view2.Close()
} catch {
    Write-Host "ERROR: $($_.Exception.Message)"
    Write-Host "HRESULT: $($_.Exception.HResult)"
}
