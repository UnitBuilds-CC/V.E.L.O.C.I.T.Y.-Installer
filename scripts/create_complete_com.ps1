# Create a complete MSI using Windows Installer COM automation
$ErrorActionPreference = "Continue"

$msiPath = "C:\temp\com_complete.msi"
if (Test-Path $msiPath) { Remove-Item $msiPath -Force }

Start-Sleep -Seconds 2

$installer = New-Object -ComObject WindowsInstaller.Installer
$db = $installer.OpenDatabase($msiPath, 3)
Write-Host "Database created"

function RunSQL($sql) {
    $v = $db.OpenView($sql)
    $v.Execute()
    $v.Close()
}

# Create tables
RunSQL "CREATE TABLE ``Property`` (``Property`` CHAR(72) NOT NULL, ``Value`` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY ``Property``)"
RunSQL "CREATE TABLE ``Directory`` (``Directory`` CHAR(72) NOT NULL, ``Directory_Parent`` CHAR(72), ``DefaultDir`` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY ``Directory``)"
RunSQL "CREATE TABLE ``Component`` (``Component`` CHAR(72) NOT NULL, ``ComponentId`` CHAR(38), ``Directory_`` CHAR(72) NOT NULL, ``Attributes`` SHORT NOT NULL, ``Condition`` CHAR(255), ``KeyPath`` CHAR(72) PRIMARY KEY ``Component``)"
RunSQL "CREATE TABLE ``Feature`` (``Feature`` CHAR(38) NOT NULL, ``Feature_Parent`` CHAR(38), ``Title`` CHAR(64) LOCALIZABLE, ``Description`` CHAR(255) LOCALIZABLE, ``Display`` SHORT, ``Level`` SHORT NOT NULL, ``Directory_`` CHAR(72), ``Attributes`` SHORT NOT NULL PRIMARY KEY ``Feature``)"
RunSQL "CREATE TABLE ``FeatureComponents`` (``Feature_`` CHAR(38) NOT NULL, ``Component_`` CHAR(72) NOT NULL PRIMARY KEY ``Feature_``, ``Component_``)"
RunSQL "CREATE TABLE ``InstallExecuteSequence`` (``Action`` CHAR(72) NOT NULL, ``Condition`` CHAR(255), ``Sequence`` SHORT PRIMARY KEY ``Action``)"
RunSQL "CREATE TABLE ``InstallUISequence`` (``Action`` CHAR(72) NOT NULL, ``Condition`` CHAR(255), ``Sequence`` SHORT PRIMARY KEY ``Action``)"
Write-Host "Tables created"

# Insert data using SQL strings
$productCode = [System.Guid]::NewGuid().ToString("B").ToUpper()
$upgradeCode = [System.Guid]::NewGuid().ToString("B").ToUpper()

RunSQL "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductName', 'Velocity Test')"
RunSQL "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductVersion', '1.0.0')"
RunSQL "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('Manufacturer', 'Velocity Corp')"
RunSQL "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductCode', '$productCode')"
RunSQL "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('UpgradeCode', '$upgradeCode')"
RunSQL "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductLanguage', '1033')"
Write-Host "Properties inserted"

RunSQL "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('TARGETDIR', '', 'SourceDir')"
RunSQL "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('ProgramFilesFolder', 'TARGETDIR', 'PFiles')"
RunSQL "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('INSTALLDIR', 'ProgramFilesFolder', 'VelocityTest')"
Write-Host "Directories inserted"

RunSQL "INSERT INTO ``Component`` (``Component``, ``Directory_``, ``Attributes``) VALUES ('MainComponent', 'INSTALLDIR', 0)"
Write-Host "Component inserted"

RunSQL "INSERT INTO ``Feature`` (``Feature``, ``Title``, ``Level``, ``Attributes``) VALUES ('MainFeature', 'Complete', 1, 0)"
Write-Host "Feature inserted"

RunSQL "INSERT INTO ``FeatureComponents`` (``Feature_``, ``Component_``) VALUES ('MainFeature', 'MainComponent')"
Write-Host "FeatureComponents inserted"

# InstallExecuteSequence
$execSeq = @(
    @("AppSearch", "", 100),
    @("CostInitialize", "", 800),
    @("FileCost", "", 900),
    @("CostFinalize", "", 1000),
    @("InstallValidate", "", 1400),
    @("InstallInitialize", "", 1500),
    @("ProcessComponents", "", 1600),
    @("UnpublishComponents", "", 1700),
    @("UnpublishFeatures", "", 1800),
    @("RemoveFiles", "", 3500),
    @("InstallFiles", "", 4000),
    @("PublishComponents", "", 6200),
    @("PublishFeatures", "", 6300),
    @("RegisterProduct", "", 6400),
    @("InstallFinalize", "", 6600)
)
foreach ($a in $execSeq) {
    if ($a[1] -eq "") {
        RunSQL "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Sequence``) VALUES ('$($a[0])', $($a[2]))"
    } else {
        RunSQL "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('$($a[0])', '$($a[1])', $($a[2]))"
    }
}

# InstallUISequence
$uiSeq = @(
    @("AppSearch", "", 100),
    @("CostInitialize", "", 800),
    @("FileCost", "", 900),
    @("CostFinalize", "", 1000),
    @("ExecuteAction", "", 1300)
)
foreach ($a in $uiSeq) {
    if ($a[1] -eq "") {
        RunSQL "INSERT INTO ``InstallUISequence`` (``Action``, ``Sequence``) VALUES ('$($a[0])', $($a[2]))"
    } else {
        RunSQL "INSERT INTO ``InstallUISequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('$($a[0])', '$($a[1])', $($a[2]))"
    }
}
Write-Host "Sequences inserted"

$db.Commit()
Write-Host "Database committed"

# Set SummaryInfo
try {
    $si = $db.SummaryInformation(20)
    $si.SetProperty(1, "Velocity Test Installation")
    $si.SetProperty(2, "Velocity Test")
    $si.SetProperty(4, "Velocity Corp")
    $si.SetProperty(7, "Intel;1033")
    $si.SetProperty(9, [System.Guid]::NewGuid().ToString("B"))
    $si.SetProperty(14, 0)
    $si.SetProperty(15, 2)
    $si.SetProperty(19, "")
    $si.Persist()
    Write-Host "SummaryInfo set"
    $db.Commit()
} catch {
    Write-Host "SummaryInfo error: $($_.Exception.Message)"
}

$size = (Get-Item $msiPath).Length
Write-Host "`nCreated: $msiPath ($size bytes)"

# Test with msiexec
$logPath = "C:\temp\com_complete.log"
if (Test-Path $logPath) { Remove-Item $logPath -Force }
$proc = Start-Process msiexec -ArgumentList "/i `"$msiPath`" /qn /l*v `"$logPath`"" -Wait -PassThru
Write-Host "msiexec exit code: $($proc.ExitCode)"

if (Test-Path $logPath) {
    Get-Content $logPath | Select-String -Pattern "Error|error|return value 3|Could not|Product:|successful|Installation|2203|2219|1619|1603" | Select-Object -First 15
}
