param (
    [Parameter(Mandatory=$true)]
    [string]$NewVersion
)

$Today = Get-Date -Format "yyyy.MM.dd"
$RootDir = $PSScriptRoot + "\.."

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

# 4. App.tsx
$AppFile = "$RootDir\src\App.tsx"
if (Test-Path $AppFile) {
    # Version X.X.X (YYYY.MM.DD) の形式を置換
    (Get-Content $AppFile) -replace 'Version [\d\.]+ \(\d{4}\.\d{2}\.\d{2}\)', "Version $NewVersion ($Today)" | Set-Content $AppFile
    Write-Host "Updated: src\App.tsx"
}

Write-Host "Version update complete!" -ForegroundColor Green
