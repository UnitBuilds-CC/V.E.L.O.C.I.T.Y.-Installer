; Inno Setup 7 script for Sample App — benchmark comparison with Velocity
; Build: iscc innosetup-sample-app.iss

#define MyAppName "Sample App"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "Velocity Team"
#define MyAppExeName "sample-app.exe"

[Setup]
AppId={{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\SampleApp
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
; Compression settings for fair comparison
Compression=lzma2/ultra64
SolidCompression=yes
; Output
OutputDir=output
OutputBaseFilename=sample-app-inno-setup
; UI
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
; Uninstall
UninstallDisplayIcon={app}\{#MyAppExeName}

[Files]
; Core application
Source: "files\bin\*"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "files\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "files\sdk\*"; DestDir: "{app}\sdk"; Flags: ignoreversion
Source: "files\samples\*"; DestDir: "{app}\samples"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\bin\{#MyAppExeName}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{commondesktop}\{#MyAppName}"; Filename: "{app}\bin\{#MyAppExeName}"

[Registry]
Root: HKLM; Subkey: "Software\SampleApp"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"
Root: HKLM; Subkey: "Software\SampleApp"; ValueType: string; ValueName: "Version"; ValueData: "{#MyAppVersion}"
Root: HKCU; Subkey: "Software\SampleApp\Settings"; ValueType: string; ValueName: "FirstRun"; ValueData: "1"

[Run]
Filename: "{app}\bin\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
