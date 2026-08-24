$installer = New-Object -ComObject WindowsInstaller.Installer
Write-Host "Methods on Installer:"
$installer.GetType() | Get-Member -MemberType Method | ForEach-Object { Write-Host "  $($_.Name)" }
