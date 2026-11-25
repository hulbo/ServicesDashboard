; Tauri NSIS 설치 프로그램 후크 (tauri.conf.json → bundle.windows.nsis.installerHooks)

; 바탕화면 바로가기를 설치하자마자 만든다.
; 기본 설치 프로그램은 마침 화면의 '바탕화면 바로가기 만들기'를 눌러야 만들고, 앱 안 업데이트(/UPDATE)에서는 만들지 않는다.
; - 직접 설치할 때는 늘 만든다 (이미 있으면 덮어씀).
; - 앱 안 업데이트에서는 아직 한 번도 만든 적이 없을 때만 만든다. 사용자가 지운 바로가기를 업데이트마다 되살리지 않는다.
; - /NS(바로가기 없이 설치)를 주면 만들지 않는다.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $NoShortcutMode <> 1
    ReadRegStr $R9 SHCTX "${MANUPRODUCTKEY}" "DesktopShortcut"
    ${If} $UpdateMode <> 1
    ${OrIf} $R9 == ""
      CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
      WriteRegStr SHCTX "${MANUPRODUCTKEY}" "DesktopShortcut" "1"
    ${EndIf}
  ${EndIf}
!macroend
