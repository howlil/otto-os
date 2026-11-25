#ifndef MyAppVersion
  #define MyAppVersion "1.3.1"
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
Compression=zip
SolidCompression=no
VersionInfoVersion={#MyAppVersion}
VersionInfoCompany=Otto OS Contributors
VersionInfoDescription=Otto OS Installer
VersionInfoProductName=Otto OS
VersionInfoProductVersion={#MyAppVersion}
VersionInfoOriginalFileName=otto-os-setup-{#MyAppVersion}-x64.exe
VersionInfoCopyright=Copyright (c) 2026 Otto OS Contributors (MIT)
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
Filename: "{app}\otto-os.exe"; Flags: nowait skipifnotsilent; Check: HasRelaunchSwitch

[Code]
const
  WM_CLOSE = $0010;
  INSTANCE_MUTEX = 'Local\OttoNotchBar';
  BAR_WINDOW_CLASS = 'OttoNotchBar';

function HasRelaunchSwitch: Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
  begin
    if CompareText(ParamStr(I), '/RELAUNCH') = 0 then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

procedure QuitRunningInstance;
var
  Wnd: HWND;
  PostedWnd: HWND;
  Attempt: Integer;
begin
  PostedWnd := 0;
  for Attempt := 0 to 199 do
  begin
    Wnd := FindWindowByClassName(BAR_WINDOW_CLASS);
    if (Wnd = 0) and (not CheckForMutexes(INSTANCE_MUTEX)) then
      Exit;
    if (Wnd <> 0) and (Wnd <> PostedWnd) then
    begin
      PostMessage(Wnd, WM_CLOSE, 0, 0);
      PostedWnd := Wnd;
    end;
    Sleep(25);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  QuitRunningInstance;
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    QuitRunningInstance;
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'OttoOS');
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Otto OS');
  end;
end;
