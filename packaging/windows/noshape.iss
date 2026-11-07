; noshape's Windows installer (Inno Setup 6), built by .github/workflows/release.yml:
;   iscc /DVersion=<version> /DSource=<folder with noshape.exe> /DOutput=<folder> packaging\windows\noshape.iss
; Installs noshape.exe (the app, the CLI and the MCP server `noshape mcp`) and noshape.com (its console launcher) to
; Program Files, adds a Start menu entry, puts the install folder on the user's PATH and, on the finish page, offers to
; connect noshape to the coding agents found (`noshape agents connect`: Claude Code, Codex CLI, Claude Desktop).
; Uninstalling disconnects them and takes the folder off PATH.

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
; Copies running from the install folder (a coding agent's `noshape mcp`) are closed, never left to fail the install
; (exit code 5, which v0.4.28-0.4.29's update helper met in silence). PrepareToInstall closes them first when silent.
CloseApplications=force
RestartApplications=no
SetupLogging=yes

[Files]
Source: "{#Source}\noshape.exe"; DestDir: "{app}"; Flags: ignoreversion
; The console launcher a terminal runs for `noshape` (.com before .exe): it waits for noshape.exe, so commands print
; before the prompt returns (crates/launcher). The Start menu and file opens use noshape.exe (no console).
Source: "{#Source}\noshape-launcher.exe"; DestDir: "{app}"; DestName: "noshape.com"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\noshape"; Filename: "{app}\noshape.exe"

[Registry]
; The install folder on the user's PATH: `noshape` and `noshape mcp` in any new terminal or MCP client.
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Run]
Filename: "{app}\noshape.exe"; Parameters: "agents connect"; Description: "Connect noshape to {code:FoundAgents} (its MCP server)"; Flags: postinstall runasoriginaluser runhidden waituntilterminated; Check: AgentsFound
Filename: "{app}\noshape.exe"; Description: "Start noshape"; Flags: postinstall nowait skipifsilent runasoriginaluser

[UninstallRun]
; Before the files go: the agents this install connected forget it (another noshape's entries stay).
Filename: "{app}\noshape.exe"; Parameters: "agents disconnect --command ""{app}\noshape.exe"""; Flags: runhidden waituntilterminated; RunOnceId: "DisconnectAgents"

[Code]
var
  Found: String;
  Checked: Boolean;
  StatusRead: Boolean;

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
      StatusRead := True;
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

// The last page says what was set up, also when there was nothing to connect (it said nothing before 11-04).
procedure CurPageChanged(CurPageID: Integer);
var
  Note: String;
  Grown: Integer;
begin
  if CurPageID <> wpFinished then
    exit;
  Note := 'The noshape command is on your PATH (in a new terminal window).';
  if AgentsFound then
    Note := Note + #13#10#13#10 + 'Coding agents found: ' + Found + '. Leave the box below ticked to connect them.'
  else if StatusRead then
    Note := Note + #13#10#13#10 + 'No coding agent was found (Claude Code, Codex CLI, Claude Desktop). After installing one, connect it from noshape: Help > Connect to coding agents...'
  else
    Note := Note + #13#10#13#10 + 'Setup could not check for coding agents. Connect them from noshape: Help > Connect to coding agents...';
  WizardForm.FinishedLabel.Caption := WizardForm.FinishedLabel.Caption + #13#10#13#10 + Note;
  Grown := WizardForm.AdjustLabelHeight(WizardForm.FinishedLabel);
  WizardForm.RunList.Top := WizardForm.RunList.Top + Grown;
  WizardForm.RunList.Height := WizardForm.RunList.Height - Grown;
end;

// A silent install (the in-app update) first ends every noshape running from the install folder, whoever started it
// and however elevated: Windows won't replace a running exe, and an update helper can't end an elevated copy.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
  Dir: String;
begin
  Result := '';
  if not WizardSilent then
    exit;
  Dir := ExpandConstant('{app}');
  StringChangeEx(Dir, '''', '''''', True);
  Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$d = ''' + Dir + '''; foreach ($i in 1..40) { $p = @(Get-Process -Name noshape -ErrorAction SilentlyContinue | Where-Object { $_.Path -and (Split-Path $_.Path) -eq $d }); if ($p.Count -eq 0) { exit 0 }; $p | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 250 }; exit 1"', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Log('Closing noshape copies in ' + ExpandConstant('{app}') + ': exit ' + IntToStr(Code));
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
