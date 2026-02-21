# Tauri NSIS Installer Template

!define APP_NAME "ChuwitchWindow"
!define CONFIG_DIR "$APPDATA\kotorichun\chuwitchwindow"

# Tauri default template include
!include "Tauri.nsh"

# カスタムアンインストーラー処理
# アンインストール完了後に呼ばれる
Section "-CustomUninstaller"
    # 設定ファイルの削除確認
    MessageBox MB_YESNO|MB_ICONQUESTION "設定ファイル（config.json 等）も削除しますか？" /SD IDNO IDNO skip_config_deletion
        RMDir /r "${CONFIG_DIR}"
        DetailPrint "設定ファイルを削除しました: ${CONFIG_DIR}"
    skip_config_deletion:
SectionEnd
