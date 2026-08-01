!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef OUTFILE
  !define OUTFILE "pinora-setup.exe"
!endif

Name "Pinora"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Pinora"
RequestExecutionLevel user
Unicode True

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Pinora"
  SetOutPath "$INSTDIR"
  File "target\package-stage\windows\pinora.exe"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$DESKTOP\Pinora.lnk" "$INSTDIR\pinora.exe"
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\Pinora.lnk"
  Delete "$INSTDIR\pinora.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
