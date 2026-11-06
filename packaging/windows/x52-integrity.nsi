; X52 Integrity NSIS installer.
; Build: makensis /DVERSION=0.2.0 /DRELEASEDIR=..\..\target\release x52-integrity.nsi
; Sign the resulting EXE with signtool after Authenticode cert is available.
;
; Silent:  X52-Integrity-Setup.exe /S
; Prefix:  X52-Integrity-Setup.exe /D=C:\Program Files\X52 Integrity

!ifndef VERSION
  !define VERSION "0.2.0"
!endif
!ifndef RELEASEDIR
  !define RELEASEDIR "..\..\target\release"
!endif

Name "X52 Integrity"
OutFile "..\..\dist\X52-Integrity-${VERSION}-setup.exe"
Unicode true
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\X52Integrity"
SetCompressor /SOLID lzma
ManifestDPIAware true
ShowInstDetails show
ShowUninstDetails show

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\orange-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\orange-uninstall.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "X52 Integrity"
VIAddVersionKey "FileDescription" "Saitek/Logitech X52 companion (HVCI-safe)"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "2026 OrbitingAncient"
VIAddVersionKey "CompanyName" "OrbitingAncient"

Var /GLOBAL LogFile

Function .onInit
  CreateDirectory "$INSTDIR\logs"
  StrCpy $LogFile "$INSTDIR\logs\install.log"
FunctionEnd

Function WriteLog
  Exch $0
  FileOpen $1 $LogFile a
  FileSeek $1 0 END
  FileWrite $1 "$0$\r$\n"
  FileClose $1
  Pop $0
FunctionEnd

Section "Install"
  SetOutPath "$INSTDIR"
  CreateDirectory "$INSTDIR\logs"

  Push "Install start ${VERSION} -> $INSTDIR"
  Call WriteLog

  File "${RELEASEDIR}\x52-integrity.exe"
  File /nonfatal "${RELEASEDIR}\x52-monitor.exe"
  CopyFiles /SILENT "$INSTDIR\x52-integrity.exe" "$INSTDIR\x52-firmfix.exe"

  SetOutPath "$INSTDIR\backups\sai075c"
  File /nonfatal /r "..\..\tools\backups\sai075c\*.*"

  SetOutPath "$INSTDIR"
  FileOpen $0 "$INSTDIR\install-manifest.json" w
  FileWrite $0 '{$\r$\n'
  FileWrite $0 '  "version": "${VERSION}",$\r$\n'
  FileWrite $0 '  "install_dir": "$INSTDIR",$\r$\n'
  FileWrite $0 '  "shortcuts_ok": true$\r$\n'
  FileWrite $0 '}$\r$\n'
  FileClose $0

  CreateDirectory "$SMPROGRAMS\X52 Integrity"
  CreateShortCut "$SMPROGRAMS\X52 Integrity\X52 Integrity.lnk" "$INSTDIR\x52-integrity.exe" "" "$INSTDIR\x52-integrity.exe" 0
  CreateShortCut "$DESKTOP\X52 Integrity.lnk" "$INSTDIR\x52-integrity.exe" "" "$INSTDIR\x52-integrity.exe" 0

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity" "DisplayName" "X52 Integrity"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity" "Publisher" "OrbitingAncient"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity" "NoRepair" 1

  Push "Install complete"
  Call WriteLog
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\X52 Integrity.lnk"
  Delete "$SMPROGRAMS\X52 Integrity\X52 Integrity.lnk"
  RMDir "$SMPROGRAMS\X52 Integrity"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\X52Integrity"
  RMDir /r "$INSTDIR"
SectionEnd
