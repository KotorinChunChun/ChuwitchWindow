param (
    [Parameter(Mandatory = $true)]
    [string]$NewVersion
)

if ($NewVersion -notmatch '^\d+\.\d+\.\d+$') {
    throw "バージョンは MAJOR.MINOR.PATCH 形式で指定してください: $NewVersion"
}

$Today = Get-Date -Format "yyyy.MM.dd"
$RootDir = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

Write-Host "Updating version to: $NewVersion" -ForegroundColor Cyan
Write-Host "Date: $Today" -ForegroundColor Cyan

# 1. Cargo.toml
$CargoFile = "$RootDir\src-tauri\Cargo.toml"
if (Test-Path $CargoFile) {
    (Get-Content $CargoFile) -replace '^version = "[^"]+"', "version = `"$NewVersion`"" | Set-Content $CargoFile
    Write-Host "Updated: src-tauri\Cargo.toml"
}

# 2. tauri.conf.json
$TauriFile = "$RootDir\src-tauri\tauri.conf.json"
if (Test-Path $TauriFile) {
    (Get-Content $TauriFile) -replace '"version": "[^"]+"', "`"version`": `"$NewVersion`"" | Set-Content $TauriFile
    Write-Host "Updated: src-tauri\tauri.conf.json"
}

# 3. package.json
$PackageFile = "$RootDir\package.json"
if (Test-Path $PackageFile) {
    (Get-Content $PackageFile) -replace '"version": "[^"]+"', "`"version`": `"$NewVersion`"" | Set-Content $PackageFile
    Write-Host "Updated: package.json"
}

# 4. package-lock.json は package.json と整合させる
Push-Location $RootDir
try {
    npm install --package-lock-only --ignore-scripts
    if ($LASTEXITCODE -ne 0) {
        throw "package-lock.json の更新に失敗しました。"
    }
}
finally {
    Pop-Location
}

Write-Host "バージョン更新が完了しました。" -ForegroundColor Green
