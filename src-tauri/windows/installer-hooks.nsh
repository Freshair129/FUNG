; FUNG NSIS installer hooks (wired through bundle.windows.nsis.installerHooks).
;
; NSIS only overwrites files it installs, so an upgrade used to leave the
; previous version's resources behind - for example a whole Whisper model the
; new version no longer ships. The directories below are bundle resources owned
; entirely by the installer, so they are replaced wholesale on upgrade. Nothing
; else in $INSTDIR is touched, and user data lives in %APPDATA%, not here.

!macro NSIS_HOOK_PREINSTALL
  ; Tauri runs this hook before its own running-app check. Run the check first
  ; so a running FUNG can never leave a half-deleted install behind.
  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

  ; Only clean an existing FUNG install; a fresh install has nothing stale.
  ${If} $INSTDIR != ""
  ${AndIf} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    RMDir /r "$INSTDIR\.venv-whisper"
    RMDir /r "$INSTDIR\knowledge-parser-runtime"
    RMDir /r "$INSTDIR\scripts"
  ${EndIf}
!macroend
