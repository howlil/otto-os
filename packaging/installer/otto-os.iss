#ifndef MyAppVersion
  #define MyAppVersion "1.2.0"
#endif

#ifndef SourceExe
  #define SourceExe "..\..\target\release\otto-os.exe"
#endif

#ifndef RepoRoot
  #define RepoRoot "..\.."
#endif

[Setup]
AppId={{8E9E4C5E-31D4-4E53-9A4C-5A8D172B0F1A}
AppName=Otto OS
AppVersion={#MyAppVersion}
AppVerName=Otto OS {#MyAppVersion}
AppPublisher=howlil
AppPublisherURL=https://github.com/howlil/otto-os
AppSupportURL=https://github.com/howlil/otto-os/issues
AppUpdatesURL=https://github.com/howlil/otto-os/releases
DefaultDirName={localappdata}\Programs\Otto OS
DisableProgramGroupPage=yes
LicenseFile={#RepoRoot}\LICENSE
OutputDir={#RepoRoot}\artifacts\installer
OutputBaseFilename=otto-os-setup-{#MyAppVersion}-x64
SetupIconFile={#RepoRoot}\crates\app\assets\otto-os.ico
UninstallDisplayIcon={app}\otto-os.exe
UninstallDisplayName=Otto OS
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22621
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "autostart"; Description: "Launch Otto OS automatically when signing in"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Otto OS"; Filename: "{app}\otto-os.exe"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "OttoOS"; ValueData: """{app}\otto-os.exe"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\otto-os.exe"; Description: "Launch Otto OS"; Flags: nowait postinstall skipifsilent

[Code]
procedure QuitRunningInstance(const ExePath: string);
var
  ResultCode: Integer;
begin
  if FileExists(ExePath) then
    Exec(ExePath, '--quit', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  ExtractTemporaryFile('otto-os.exe');
  QuitRunningInstance(ExpandConstant('{tmp}\otto-os.exe'));
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    QuitRunningInstance(ExpandConstant('{app}\otto-os.exe'));
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'OttoOS');
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Otto OS');
  end;
end;
