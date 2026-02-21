!macro NSIS_HOOK_PREUNINSTALL
    MessageBox MB_YESNO|MB_ICONQUESTION "ChuwitchWindow をアンインストールしますか？" /SD IDYES IDYES +2
        Quit

    # アンインストール時にスタートアップ（タスクスケジューラ）登録を削除
    nsExec::ExecToStack 'schtasks /Delete /F /TN "ChuwitchWindow_AutoStart"'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
    # 設定ファイルの場所定数
    # config.rs で ProjectDirs::from("com", "kotorichun", "chuwitchwindow") を使用しているため、
    # Windows では $APPDATA\kotorichun\chuwitchwindow となる
    StrCpy $1 "$APPDATA\kotorichun\chuwitchwindow"

    # 設定ファイルの削除確認ダイアログ
    MessageBox MB_YESNO|MB_ICONQUESTION "設定ファイル（config.json 等）も削除しますか？" /SD IDNO IDNO skip_config_deletion
        RMDir /r "$1"
        DetailPrint "設定ファイルを削除しました: $1"
    skip_config_deletion:
!macroend
