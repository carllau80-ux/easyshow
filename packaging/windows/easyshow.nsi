; EasyShow Windows 安装包(NSIS 3)
; 用户级安装($LOCALAPPDATA\Programs\EasyShow[ Lite]),无需管理员权限。
; 由 packaging/windows/make-package.sh 调用:
;   makensis -DVERSION=x.y.z -DVARIANT=full|lite -DSRC=<payload> -DOUT=<setup.exe> [-DICON=<icon.ico>] easyshow.nsi
;
; 两个变体可共存:安装目录、注册表键、ProgID、"打开方式"登记、快捷方式均按变体区分。

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef VARIANT
  !define VARIANT "full"
!endif
!ifndef SRC
  !error "SRC 未定义(payload 目录)"
!endif
!ifndef OUT
  !define OUT "EasyShow-Setup.exe"
!endif

; ---- 变体参数 ----
!if "${VARIANT}" == "lite"
  !define PRODUCT_NAME "EasyShow Lite"
  !define EXE_NAME "easyshow-lite.exe"
  !define REGKEY "EasyShowLite"
  !define PROGID "EasyShowLite.Image"
  !define PAYLOAD_EXE "easyshow-lite.exe"
!else
  !define PRODUCT_NAME "EasyShow"
  !define EXE_NAME "easyshow.exe"
  !define REGKEY "EasyShow"
  !define PROGID "EasyShow.Image"
  !define PAYLOAD_EXE "easyshow.exe"
!endif
!define DISPLAY_NAME "${PRODUCT_NAME} 图片查看器"

Name "${DISPLAY_NAME}"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT_NAME}"
InstallDirRegKey HKCU "Software\${REGKEY}" "InstallDir"
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

Section "${DISPLAY_NAME} 主程序 (必需)" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${SRC}${PAYLOAD_EXE}"
  File /nonfatal "${SRC}README.md"
  File /nonfatal "${SRC}THIRD-PARTY.md"
  File /nonfatal "${SRC}使用说明.txt"
  File /nonfatal "${SRC}LICENSE-MIT" "${SRC}LICENSE-APACHE"
  File /nonfatal /r "${SRC}*.dll"
  SetOutPath "$INSTDIR\licenses"
  File /nonfatal /r "${LICENSES}*.*"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "Software\${REGKEY}" "InstallDir" "$INSTDIR"

  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${DISPLAY_NAME}.lnk" "$INSTDIR\${EXE_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\卸载 ${PRODUCT_NAME}.lnk" "$INSTDIR\uninstall.exe"

  ; 用户级 ProgID:加入"打开方式",不劫持系统默认(默认由用户在系统设置里确认)
  WriteRegStr HKCU "Software\Classes\${PROGID}" "" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\${EXE_NAME},0"
  WriteRegStr HKCU "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\${EXE_NAME}" "%1"'
  WriteRegStr HKCU "Software\Classes\Applications\${EXE_NAME}" "FriendlyAppName" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Classes\Applications\${EXE_NAME}\DefaultIcon" "" "$INSTDIR\${EXE_NAME},0"
  WriteRegStr HKCU "Software\Classes\Applications\${EXE_NAME}\shell\open\command" "" '"$INSTDIR\${EXE_NAME}" "%1"'

  ; 控制面板"应用与功能"条目
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "DisplayName" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "DisplayIcon" "$INSTDIR\${EXE_NAME},0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "Publisher" "EasyShow"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}" "EstimatedSize" "$0"
SectionEnd

Section "桌面快捷方式" SecDesktop
  CreateShortCut "$DESKTOP\${DISPLAY_NAME}.lnk" "$INSTDIR\${EXE_NAME}"
SectionEnd

Section "关联图片格式(加入 打开方式)" SecAssoc
  !macro AssocExt EXT
    WriteRegStr HKCU "Software\Classes\${EXT}\OpenWithProgids" "${PROGID}" ""
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
!if "${VARIANT}" == "full"
  !insertmacro AssocExt ".heic"
  !insertmacro AssocExt ".heif"
  !insertmacro AssocExt ".avif"
!endif
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
!if "${VARIANT}" == "lite"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "EasyShow Lite 主程序(轻量版,不含 HEIC/AVIF 支持)"
!else
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "EasyShow 主程序与 HEIC/AVIF 支持库"
!endif
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "在桌面创建快捷方式"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAssoc} "把 ${PRODUCT_NAME} 加入常见图片格式的“打开方式”列表"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\easyshow.exe"
  Delete "$INSTDIR\easyshow-lite.exe"
  Delete "$INSTDIR\*.dll"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\使用说明.txt"
  Delete "$INSTDIR\LICENSE-MIT"
  Delete "$INSTDIR\LICENSE-APACHE"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\licenses"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${PRODUCT_NAME}\${DISPLAY_NAME}.lnk"
  Delete "$SMPROGRAMS\${PRODUCT_NAME}\卸载 ${PRODUCT_NAME}.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT_NAME}"
  Delete "$DESKTOP\${DISPLAY_NAME}.lnk"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGKEY}"
  DeleteRegKey HKCU "Software\${REGKEY}"
  DeleteRegKey HKCU "Software\Classes\${PROGID}"
  DeleteRegKey HKCU "Software\Classes\Applications\${EXE_NAME}"
  !macro DelAssoc EXT
    DeleteRegValue HKCU "Software\Classes\${EXT}\OpenWithProgids" "${PROGID}"
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
