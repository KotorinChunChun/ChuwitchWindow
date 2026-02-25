# GitHub Release Script for ChuwitchWindow

# 1. バージョン情報を Cargo.toml から取得
$cargoToml = Get-Content "src-tauri/Cargo.toml" -Raw
if ($cargoToml -match 'version\s*=\s*"([^"]+)"') {
    $version = $Matches[1]
}
else {
    Write-Error "Cargo.toml からバージョンを取得できませんでした。"
    exit 1
}

$releaseName = "ChuwitchWindow_v$version"
$releaseDir = "release/$releaseName"
$zipFile = "release/$releaseName.zip"

Write-Host "Releasing $releaseName ..." -ForegroundColor Cyan

# バージョンを確認してから続行するか問い合わせる
$confirm = Read-Host "バージョン v$version でリリースを開始します。続行しますか？ (y/N)"
if ($confirm -notmatch '^[Yy]$') {
    Write-Host "キャンセルしました。" -ForegroundColor Gray
    exit 0
}

# 2. Tauri ビルドの実行 (NSIS インストーラーを含む)
Write-Host "Building Tauri app..." -ForegroundColor Yellow
npm run tauri build -- --bundles nsis
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

# 実行ファイル (src-tauri/target/release/chuwitchwindow.exe -> ChuwitchWindow.exe)
$exePath = "src-tauri/target/release/chuwitchwindow.exe"
if (Test-Path $exePath) {
    Copy-Item $exePath "$releaseDir/ChuwitchWindow.exe"
}
else {
    Write-Warning "実行ファイルが見つかりません: $exePath"
}

# インストーラー (NSIS)
# Tauri v2 のデフォルト出力先: src-tauri/target/release/bundle/nsis/ChuwitchWindow_<version>_x64-setup.exe
$installerPath = "src-tauri/target/release/bundle/nsis/ChuwitchWindow_$($version)_x64-setup.exe"
$setupDestPath = "release/ChuwitchWindow_v$($version)_Setup.exe"
if (Test-Path $installerPath) {
    Copy-Item $installerPath $setupDestPath
}
else {
    # フォルダを探索して最新のインストーラーを探す
    $searchPattern = "src-tauri/target/release/bundle/nsis/*.exe"
    $foundInstaller = Get-ChildItem -Path $searchPattern | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($foundInstaller) {
        Copy-Item $foundInstaller.FullName $setupDestPath
    }
    else {
        Write-Warning "インストーラーが見つかりません: $installerPath"
    }
}

# README.md
if (Test-Path "README.md") {
    Copy-Item "README.md" "$releaseDir/README.md"
}


# 5. ZIP 圧縮
Write-Host "Compressing to ZIP..." -ForegroundColor Yellow
if (Test-Path $zipFile) { Remove-Item $zipFile }
Compress-Archive -Path "$releaseDir/*" -DestinationPath $zipFile

# ZIP作成後、一時フォルダを削除
if (Test-Path $releaseDir) {
    Remove-Item -Recurse -Force $releaseDir
}

Write-Host "Release package created at: $zipFile" -ForegroundColor Green

# 6. GitHub Release への追加 (gh CLI がある場合)
$ghAvailable = Get-Command gh -ErrorAction SilentlyContinue
if ($ghAvailable) {
    Write-Host "Checking GitHub CLI and creating release..." -ForegroundColor Cyan
    # タグが存在するか確認、なければ作成
    $tagName = "v$version"
    gh release create $tagName $zipFile $setupDestPath --title "Release $tagName" --notes "Automated release for $tagName" --overwrite
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Successfully created GitHub release: $tagName" -ForegroundColor Green
    }
    else {
        Write-Warning "GitHub リリースの作成に失敗しました。認証（gh auth login）を確認してください。"
    }
}
else {
    Write-Host "GitHub CLI (gh) が見つからないため、GitHub へのアップロードはスキップしました。" -ForegroundColor Gray
}
