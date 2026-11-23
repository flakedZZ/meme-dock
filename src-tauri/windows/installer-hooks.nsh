; Tauri includes this hook before registering its MUI languages.
LangString InstallDirectoryNotWritable 2052 "无法写入安装目录：$INSTDIR。请为当前用户选择可写目录，或以管理员身份重新运行安装程序，并选择为所有用户安装。"
LangString InstallDirectoryNotWritable 1033 "Cannot write to the installation folder: $INSTDIR. Choose a folder your account can write to, or run Setup as administrator and install for all users."

!macro NSIS_HOOK_PREINSTALL
  Push $0
  ClearErrors
  GetTempFileName $0 "$INSTDIR"
  ${If} ${Errors}
    Pop $0
    IfSilent +2
    MessageBox MB_OK|MB_ICONSTOP "$(InstallDirectoryNotWritable)"
    SetErrorLevel 2
    Abort "$(InstallDirectoryNotWritable)"
  ${EndIf}
  Delete "$0"
  Pop $0
!macroend
