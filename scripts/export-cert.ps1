$cert = Get-ChildItem 'Cert:\CurrentUser\My\1D0E55DD4BF13E0E7A5D0E03EFD4A5D8D58CAB67'
$password = ConvertTo-SecureString -String 'REDACTED_SECRET' -Force -AsPlainText
$pfxPath = Join-Path $PSScriptRoot 'unitbuilds-codesign.pfx'
Export-PfxCertificate -Cert $cert -FilePath $pfxPath -Password $password
Write-Host "Exported to: $pfxPath"
