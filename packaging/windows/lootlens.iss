; Inno Setup script for the Windows installer. Built by the release workflow:
;   iscc /DAppVersion=0.1.0 /DSourceDir=<windeployqt folder> /DOutputDir=dist packaging\windows\lootlens.iss
; Installs per user (no admin rights) into %LOCALAPPDATA%\Programs\Loot Lens.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #error Pass /DSourceDir=<folder with lootlens.exe and its Qt files>
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
AppId={{A6A396E5-B267-41CD-ADC0-24430B9670F6}
AppName=Loot Lens
AppVersion={#AppVersion}
AppVerName=Loot Lens {#AppVersion}
AppPublisher=Loot Lens contributors
AppPublisherURL=https://github.com/rockymontana/loot-lens
AppSupportURL=https://github.com/rockymontana/loot-lens/issues
AppUpdatesURL=https://github.com/rockymontana/loot-lens/releases
DefaultDirName={autopf}\Loot Lens
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=lootlens-{#AppVersion}-windows-x64-setup
SetupIconFile=lootlens.ico
UninstallDisplayIcon={app}\lootlens.ico
UninstallDisplayName=Loot Lens
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
; Close a running Loot Lens (it sits in the tray) before upgrading or uninstalling.
CloseApplications=force

[Tasks]
Name: "autostart"; Description: "Start Loot Lens when &Windows starts"
Name: "desktopicon"; Description: "Create a &desktop shortcut"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "lootlens.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Loot Lens"; Filename: "{app}\lootlens.exe"; IconFilename: "{app}\lootlens.ico"; Comment: "Path of Exile 1 & 2 price checker"
Name: "{autodesktop}\Loot Lens"; Filename: "{app}\lootlens.exe"; IconFilename: "{app}\lootlens.ico"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Loot Lens"; ValueData: """{app}\lootlens.exe"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\lootlens.exe"; Description: "Start Loot Lens now"; Flags: nowait postinstall skipifsilent
