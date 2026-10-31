; noshape's Windows installer (Inno Setup 6), built by .github/workflows/release.yml:
;   iscc /DVersion=<version> /DSource=<folder with noshape.exe> /DOutput=<folder> packaging\windows\noshape.iss
; Installs noshape.exe (the app, the CLI and the MCP server `noshape mcp`) to Program Files, adds a Start menu entry,
; puts the install folder on the user's PATH (removed on uninstall) and, on the finish page, offers to connect noshape
; to the coding agents found (`noshape agents connect`: Claude Code, Codex CLI, Claude Desktop).

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Source
  #define Source "..\..\target\release"
#endif
#ifndef Output
  #define Output "."
#endif

[Setup]
AppId={{6E0B3C55-2D7A-4C4B-9C8E-5F1A4E2B7D10}
AppName=noshape
AppVersion={#Version}
AppVerName=noshape {#Version}
AppPublisher=noshape
AppPublisherURL=https://github.com/yekta/noshape
DefaultDirName={autopf}\noshape
DefaultGroupName=noshape
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
OutputDir={#Output}
OutputBaseFilename=noshape-{#Version}-windows-x86_64-setup
SetupIconFile=..\icons\noshape.ico
UninstallDisplayIcon={app}\noshape.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "{#Source}\noshape.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\noshape"; Filename: "{app}\noshape.exe"

[Registry]
; The install folder on the user's PATH: `noshape` and `noshape mcp` in any new terminal or MCP client.
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Run]
Filename: "{app}\noshape.exe"; Parameters: "agents connect"; Description: "Connect noshape to {code:FoundAgents} (its MCP server)"; Flags: postinstall runasoriginaluser runhidden waituntilterminated; Check: AgentsFound
Filename: "{app}\noshape.exe"; Description: "Start noshape"; Flags: postinstall nowait skipifsilent runasoriginaluser unchecked

[Code]
var
  Found: String;
  Checked: Boolean;

function NeedsAddPath(Dir: String): Boolean;
var
  Path: String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Path) + ';') = 0;
end;

// The coding agents installed, as `noshape agents status` names them ("Claude Code, Codex CLI").
function FoundAgents(Param: String): String;
var
  Code, P: Integer;
  Raw: AnsiString;
  Text, Key: String;
begin
  if not Checked then
  begin
    Checked := True;
    Found := '';
    // Piped through PowerShell, which waits for the (GUI-subsystem) program and hands it a pipe.
    if ExecAsOriginalUser('powershell.exe', '-NoProfile -NonInteractive -Command "& ''' + ExpandConstant('{app}\noshape.exe') + ''' agents status | Out-File -Encoding ascii ''' + ExpandConstant('{tmp}\agents.json') + '''"', '', SW_HIDE, ewWaitUntilTerminated, Code)
      and LoadStringFromFile(ExpandConstant('{tmp}\agents.json'), Raw) then
    begin
      Text := String(Raw);
      Key := '"found_names": "';
      P := Pos(Key, Text);
      if P > 0 then
      begin
        Text := Copy(Text, P + Length(Key), Length(Text));
        Found := Copy(Text, 1, Pos('"', Text) - 1);
      end;
    end;
  end;
  Result := Found;
end;

function AgentsFound: Boolean;
begin
  Result := FoundAgents('') <> '';
end;

// Uninstall: the install folder off the user's PATH.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Path, Dir: String;
  P: Integer;
begin
  if CurUninstallStep <> usPostUninstall then
    exit;
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path) then
    exit;
  Dir := ExpandConstant('{app}');
  Path := ';' + Path + ';';
  P := Pos(';' + Uppercase(Dir) + ';', Uppercase(Path));
  if P = 0 then
    exit;
  Delete(Path, P, Length(Dir) + 1);
  Path := Copy(Path, 2, Length(Path) - 2);
  RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path);
end;
