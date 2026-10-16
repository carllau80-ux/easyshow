; Liteshow Windows 安装包(NSIS 3)
; 用户级安装($LOCALAPPDATA\Programs\Liteshow),无需管理员权限。
; 由 packaging/windows/make-package.sh 调用:
;   makensis -DVERSION=x.y.z -DSRC=<payload> -DOUT=<setup.exe> [-DICON=<icon.ico>] liteshow.nsi

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef SRC
  !error "SRC 未定义(payload 目录)"
!endif
!ifndef OUT
  !define OUT "Liteshow-Setup.exe"
!endif

Name "Liteshow 图片查看器"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\Liteshow"
InstallDirRegKey HKCU "Software\Liteshow" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show

!ifdef ICON
  !define MUI_ICON "${ICON}"
  !define MUI_UNICON "${ICON}"
!endif

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Liteshow 主程序 (必需)" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${SRC}/liteshow.exe"
  File /nonfatal "${SRC}/README.md"
  File /nonfatal "${SRC}/使用说明.txt"
  File /nonfatal "${SRC}/LICENSE-MIT" "${SRC}/LICENSE-APACHE"
  File /nonfatal /r "${SRC}/*.dll"
  SetOutPath "$INSTDIR\licenses"
  File /nonfatal /r "${SRC}/licenses/*.*"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "Software\Liteshow" "InstallDir" "$INSTDIR"

  CreateDirectory "$SMPROGRAMS\Liteshow"
  CreateShortCut "$SMPROGRAMS\Liteshow\Liteshow 图片查看器.lnk" "$INSTDIR\liteshow.exe"
  CreateShortCut "$SMPROGRAMS\Liteshow\卸载 Liteshow.lnk" "$INSTDIR\uninstall.exe"

  ; 用户级 ProgID:加入"打开方式",不劫持系统默认(默认由用户在系统设置里确认)
  WriteRegStr HKCU "Software\Classes\Liteshow.Image" "" "Liteshow 图片查看器"
  WriteRegStr HKCU "Software\Classes\Liteshow.Image\DefaultIcon" "" "$INSTDIR\liteshow.exe,0"
  WriteRegStr HKCU "Software\Classes\Liteshow.Image\shell\open\command" "" '"$INSTDIR\liteshow.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\Applications\liteshow.exe" "FriendlyAppName" "Liteshow 图片查看器"
  WriteRegStr HKCU "Software\Classes\Applications\liteshow.exe\DefaultIcon" "" "$INSTDIR\liteshow.exe,0"
  WriteRegStr HKCU "Software\Classes\Applications\liteshow.exe\shell\open\command" "" '"$INSTDIR\liteshow.exe" "%1"'

  ; 控制面板"应用与功能"条目
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "DisplayName" "Liteshow 图片查看器"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "DisplayIcon" "$INSTDIR\liteshow.exe,0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "Publisher" "Liteshow"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow" "EstimatedSize" "$0"
SectionEnd

Section "桌面快捷方式" SecDesktop
  CreateShortCut "$DESKTOP\Liteshow 图片查看器.lnk" "$INSTDIR\liteshow.exe"
SectionEnd

Section "关联图片格式(加入 打开方式)" SecAssoc
  !macro AssocExt EXT
    WriteRegStr HKCU "Software\Classes\${EXT}\OpenWithProgids" "Liteshow.Image" ""
  !macroend
  !insertmacro AssocExt ".jpg"
  !insertmacro AssocExt ".jpeg"
  !insertmacro AssocExt ".png"
  !insertmacro AssocExt ".gif"
  !insertmacro AssocExt ".webp"
  !insertmacro AssocExt ".bmp"
  !insertmacro AssocExt ".tif"
  !insertmacro AssocExt ".tiff"
  !insertmacro AssocExt ".ico"
  !insertmacro AssocExt ".heic"
  !insertmacro AssocExt ".heif"
  !insertmacro AssocExt ".avif"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "Liteshow 主程序与 HEIC/AVIF 支持库"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "在桌面创建快捷方式"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAssoc} "把 Liteshow 加入常见图片格式的“打开方式”列表"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\liteshow.exe"
  Delete "$INSTDIR\*.dll"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\使用说明.txt"
  Delete "$INSTDIR\LICENSE-MIT"
  Delete "$INSTDIR\LICENSE-APACHE"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\licenses"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\Liteshow\Liteshow 图片查看器.lnk"
  Delete "$SMPROGRAMS\Liteshow\卸载 Liteshow.lnk"
  RMDir "$SMPROGRAMS\Liteshow"
  Delete "$DESKTOP\Liteshow 图片查看器.lnk"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Liteshow"
  DeleteRegKey HKCU "Software\Liteshow"
  DeleteRegKey HKCU "Software\Classes\Liteshow.Image"
  DeleteRegKey HKCU "Software\Classes\Applications\liteshow.exe"
  !macro DelAssoc EXT
    DeleteRegValue HKCU "Software\Classes\${EXT}\OpenWithProgids" "Liteshow.Image"
  !macroend
  !insertmacro DelAssoc ".jpg"
  !insertmacro DelAssoc ".jpeg"
  !insertmacro DelAssoc ".png"
  !insertmacro DelAssoc ".gif"
  !insertmacro DelAssoc ".webp"
  !insertmacro DelAssoc ".bmp"
  !insertmacro DelAssoc ".tif"
  !insertmacro DelAssoc ".tiff"
  !insertmacro DelAssoc ".ico"
  !insertmacro DelAssoc ".heic"
  !insertmacro DelAssoc ".heif"
  !insertmacro DelAssoc ".avif"
  RMDir "$INSTDIR"
SectionEnd
