# =============================================================================
#  リリース自動化スクリプト（テンプレート）
#  - 配置場所: dev/scripts/release-<アプリ名>.ps1
#    （原本は default プロジェクトの dev/templates/release-Sample.ps1）
#  - 処理内容: Cargo.toml のバージョン取得 → cargo build --release →
#    ファイル収集 → ZIP化 → Inno Setup でインストーラ生成 →
#    GitHub Release 作成/更新（gh があれば）→ release/Publish へ移動
#  - 使い方: 下の「リリース設定」ブロックだけをプロジェクトに合わせて書き換える。
# =============================================================================
$ErrorActionPreference = 'Stop'

# =============================================================================
#  リリース設定（★プロジェクトごとに、この節だけ書き換える）
# =============================================================================
# アプリ名（ZIP名・Setup名・実行ファイル名のベースに使用）
$AppName = "<アプリ名>"
# 実行ファイル名（通常は "$AppName.exe"）
$ExeName = "$AppName.exe"
# ZIP に同梱する追加ファイル（依存DLLなど）。target/release からの相対名。
# 存在しないものは警告を出してスキップする。不要なら空配列 @() にする。
$ExtraFiles = @(
    "DirectML.dll"
)
# Inno Setup スクリプト。存在しなければインストーラ生成をスキップする。
$InnoSetupScript = "installer.iss"
# =============================================================================

# スクリプトの存在するディレクトリ（dev/scripts/）から2階層上をプロジェクトルートとする
$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\")
Push-Location $ProjectRoot

try {
    # 1. バージョン情報を Cargo.toml から取得
    $cargoToml = Get-Content "Cargo.toml" -Raw
    if ($cargoToml -match 'version\s*=\s*"([^"]+)"') {
        $version = $Matches[1]
    }
    else {
        Write-Error "Cargo.toml からバージョンを取得できませんでした。"
        exit 1
    }

    $releaseName = "${AppName}_v$version"
    $releaseDir = "release/$releaseName"
    $zipFile = "release/$releaseName.zip"
    $setupFileName = "${AppName}_v${version}_Setup"
    $setupDestPath = "release/$setupFileName.exe"

    Write-Host "Releasing $releaseName ..." -ForegroundColor Cyan

    # バージョンを確認してから続行するか問い合わせる
    $confirm = Read-Host "バージョン v$version でリリースを開始します。続行しますか？ (y/N)"
    if ($confirm -notmatch '^[Yy]$') {
        Write-Host "キャンセルしました。" -ForegroundColor Gray
        exit 0
    }

    # 2. Rust ビルドの実行
    Write-Host "Building Rust binary..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "ビルドに失敗しました。"
        exit 1
    }

    # 3. リリースフォルダの準備
    if (Test-Path $releaseDir) {
        Remove-Item -Recurse -Force $releaseDir
    }
    New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null

    # 4. ファイルのコピー
    Write-Host "Copying files..." -ForegroundColor Yellow

    # 実行ファイル
    $exePath = "target/release/$ExeName"
    if (Test-Path $exePath) {
        Copy-Item $exePath "$releaseDir/$ExeName"
    }
    else {
        Write-Warning "実行ファイルが見つかりません: $exePath"
    }

    # 追加ファイル（依存DLLなど。存在しないものはスキップ）
    foreach ($f in $ExtraFiles) {
        $extraPath = "target/release/$f"
        if (Test-Path $extraPath) {
            Copy-Item $extraPath "$releaseDir/$f"
        }
        else {
            Write-Warning "追加ファイルが見つかりません（スキップ）: $extraPath"
        }
    }

    # README.md
    if (Test-Path "README.md") {
        Copy-Item "README.md" "$releaseDir/README.md"
    }

    # インストーラー (Inno Setup)
    # /O で出力先フォルダ、/F で出力ファイル名を直接指定
    if (Test-Path $InnoSetupScript) {
        Write-Host "Building installer with Inno Setup..." -ForegroundColor Yellow
        $isccPath = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
        if (-not (Test-Path $isccPath)) {
            $isccPath = "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
        }
        if (-not (Test-Path $isccPath)) {
            $isccPath = "C:\Program Files\Inno Setup 6\ISCC.exe"
        }

        if (Test-Path $isccPath) {
            & $isccPath /O"release" /F"$setupFileName" "$InnoSetupScript"
            if ($LASTEXITCODE -eq 0 -and (Test-Path $setupDestPath)) {
                Write-Host "インストーラーを作成しました: $setupDestPath" -ForegroundColor Green
            }
            else {
                Write-Warning "Inno Setup のコンパイルが完了しなかったか、生成ファイルが見つかりません。"
            }
        }
        else {
            Write-Warning "Inno Setup コンパイラ(ISCC.exe)が見つかりませんでした。インストーラーの生成をスキップします。"
        }
    }
    else {
        Write-Host "Inno Setup スクリプト ($InnoSetupScript) が無いため、インストーラー生成をスキップします。" -ForegroundColor Gray
    }

    # 5. ZIP 圧縮
    Write-Host "Compressing to ZIP..." -ForegroundColor Yellow
    if (Test-Path $zipFile) { Remove-Item $zipFile }
    Write-Host "Waiting for file locks to clear..." -ForegroundColor Yellow
    Start-Sleep -Seconds 3
    Compress-Archive -Path "$releaseDir/*" -DestinationPath $zipFile -ErrorAction Stop

    # ZIP作成後、一時フォルダを削除
    if (Test-Path $releaseDir) {
        Remove-Item -Recurse -Force $releaseDir
    }

    Write-Host "Release package created at: $zipFile" -ForegroundColor Green

    # 6. GitHub Release への追加 (gh CLI がある場合)
    $ghAvailable = Get-Command gh -ErrorAction SilentlyContinue
    if ($ghAvailable) {
        Write-Host "GitHub CLI の状態とリリースの存在を確認中..." -ForegroundColor Cyan
        $tagName = "v$version"

        # 既存のリリースがあるか確認
        $null = gh release view $tagName 2>&1
        $releaseExists = ($LASTEXITCODE -eq 0)

        # アップロードするアセットの集約
        $uploadAssets = @($zipFile)
        if (Test-Path $setupDestPath) {
            $uploadAssets += $setupDestPath
        }

        if ($releaseExists) {
            Write-Host "警告: GitHub リリース $tagName は既に存在します。" -ForegroundColor Yellow
            $overwrite = Read-Host "既存のリリースのファイルを上書きして更新しますか？ (y/N)"
            if ($overwrite -match '^[Yy]$') {
                Write-Host "アセットを上書きアップロード中..." -ForegroundColor Yellow
                gh release upload $tagName $uploadAssets --clobber
                if ($LASTEXITCODE -eq 0) {
                    Write-Host "GitHub リリースアセットを正常に更新しました: $tagName" -ForegroundColor Green
                }
                else {
                    Write-Warning "GitHub リリースアセットの更新に失敗しました。"
                }
            }
            else {
                Write-Host "リリースの更新をスキップしました。" -ForegroundColor Gray
            }
        }
        else {
            Write-Host "新規リリースを作成中..." -ForegroundColor Yellow
            gh release create $tagName $uploadAssets --title "Release $tagName" --notes "Automated release for $tagName"
            if ($LASTEXITCODE -eq 0) {
                Write-Host "GitHub リリースを正常に作成しました: $tagName" -ForegroundColor Green
            }
            else {
                Write-Warning "GitHub リリースの作成に失敗しました。認証（gh auth login）を確認してください。"
            }
        }
    }
    else {
        Write-Host "GitHub CLI (gh) が見つからないため、GitHub へのアップロードはスキップしました。" -ForegroundColor Gray
    }

    # 7. リリース成果物を release/Publish へ移動（Git対象外の公開控え）
    $publishDir = "release/Publish"
    if (-not (Test-Path $publishDir)) {
        New-Item -ItemType Directory -Path $publishDir -Force | Out-Null
    }

    Write-Host "Moving artifacts to $publishDir ..." -ForegroundColor Yellow
    if (Test-Path $zipFile) {
        Move-Item -Path $zipFile -Destination $publishDir -Force
    }
    if (Test-Path $setupDestPath) {
        Move-Item -Path $setupDestPath -Destination $publishDir -Force
    }

    Write-Host "全ての処理が完了しました！" -ForegroundColor Green
}
finally {
    Pop-Location
}

