; open-runo-router (RPoem) Windowsインストーラー(Inno Setup)。
;
; ユーザー指示「実行バイナリを持つリポジトリについてのみ、
; {リポジトリ名}-install.exeという名前の単一Windows実行ファイル
; インストーラーを作成」への対応。`open-english`の
; `installer/windows/open-english.iss`の実装パターンを踏襲した。
;
; 正直な開示: このインストーラーが同梱するのはこの開発機上で
; `cargo build --release -p open-runo-router`でビルドした実行ファイル
; 1個のみ。RPoemはREST APIサーバー(既定ポート8081)であり、GUIは
; 持たない——インストール後はコマンドラインまたはWindowsサービスとして
; 運用することを想定している(サービス登録は本インストーラーの
; スコープ外、`nssm`等の別ツールでの登録を別途検討すること)。
;
; ビルド方法: リポジトリルートで`cargo build --release -p
; open-runo-router`を実行した後、このディレクトリで
; `ISCC.exe open-runo-router.iss`を実行する。

#define MyAppName "open-runo-router"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-local-build"
#endif
#define MyAppPublisher "aon-co-jp"
#define MyAppURL "https://github.com/aon-co-jp/RPoem"
#define MyAppExeName "open-runo-router.exe"

[Setup]
PrivilegesRequired=lowest
AppId={{4A2D8E1F-7C93-4B5E-A210-9E6F3C7D8B45}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
OutputDir=dist
OutputBaseFilename=open-runo-router-installer
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\install.sh"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "README-INSTALLED.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
