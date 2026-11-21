; Inno Setup script for the Windows installer. Built by the release workflow:
;   iscc /DAppVersion=0.1.0 /DSourceDir=<windeployqt folder> /DOutputDir=dist packaging\windows\lootingglass.iss
; Installs per user (no admin rights) into %LOCALAPPDATA%\Programs\Looting Glass.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #error Pass /DSourceDir=<folder with lootingglass.exe and its Qt files>
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
AppId={{A6A396E5-B267-41CD-ADC0-24430B9670F6}
AppName=Looting Glass
AppVersion={#AppVersion}
AppVerName=Looting Glass {#AppVersion}
AppPublisher=Looting Glass contributors
AppPublisherURL=https://github.com/looting-glass/desktop
AppSupportURL=https://github.com/looting-glass/desktop/issues
AppUpdatesURL=https://github.com/looting-glass/desktop/releases
DefaultDirName={autopf}\Looting Glass
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=lootingglass-{#AppVersion}-windows-x64-setup
SetupIconFile=lootingglass.ico
UninstallDisplayIcon={app}\lootingglass.ico
UninstallDisplayName=Looting Glass
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
; Close a running Looting Glass (it sits in the tray) before upgrading or uninstalling.
CloseApplications=force

[Tasks]
Name: "autostart"; Description: "Start Looting Glass when &Windows starts"
Name: "desktopicon"; Description: "Create a &desktop shortcut"; Flags: unchecked

; Upgrading from Loot Lens (same AppId): remove its files and shortcuts. Its settings in
; %APPDATA%\lootlens are carried over by the app on first start.
[InstallDelete]
Type: files; Name: "{app}\lootlens.exe"
Type: files; Name: "{app}\lootlens.ico"
Type: files; Name: "{autoprograms}\Loot Lens.lnk"
Type: files; Name: "{autodesktop}\Loot Lens.lnk"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "lootingglass.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Looting Glass"; Filename: "{app}\lootingglass.exe"; IconFilename: "{app}\lootingglass.ico"; Comment: "Path of Exile 1 & 2 price checker"
Name: "{autodesktop}\Looting Glass"; Filename: "{app}\lootingglass.exe"; IconFilename: "{app}\lootingglass.ico"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Loot Lens"; Flags: deletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Looting Glass"; ValueData: """{app}\lootingglass.exe"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\lootingglass.exe"; Description: "Start Looting Glass now"; Flags: nowait postinstall skipifsilent
; In-app update: the app runs this installer with /SILENT /relaunch=1 and exits; start it again.
Filename: "{app}\lootingglass.exe"; Flags: nowait skipifnotsilent; Check: Relaunch

[Code]
function Relaunch: Boolean;
begin
  Result := ExpandConstant('{param:relaunch|0}') = '1';
end;
