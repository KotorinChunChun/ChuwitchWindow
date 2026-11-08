[CmdletBinding()]
param(
    [ValidateSet('All', 'Drag', 'MonitorMovement', 'Arrange', 'Rules', 'Settings', 'Pin', 'Hotkeys', 'Frontend', 'Runtime')]
    [string]$Feature = 'All',
    [string]$LogDirectory,
    [string]$RustSourceRoot = (Join-Path $PSScriptRoot '../..'),
    [int]$RunningProcessId,
    [switch]$List
)

$ErrorActionPreference = 'Stop'
$rustFilters = [ordered]@{
    All = @('')
    Drag = @('alt_drag::')
    MonitorMovement = @('action::', 'logic::', 'window::')
    Arrange = @('arrange::', 'action::tests::cli_accepts_all_arrange_modes_and_rejects_unknown_modes')
    Rules = @('rule::')
    Settings = @('config::')
    Pin = @('pin::')
    Hotkeys = @('hotkey::')
    Frontend = @()
    Runtime = @()
}
$manualChecks = @(
    '中ボタン実入力からウィンドウ移動までの通し操作とアプリ本来の中クリック'
    '実モニター構成での巡回・入れ替え・退避・集約・整列・元に戻す'
    'ホットキーの実入力、モニターの着脱、通知領域と設定画面の表示・操作'
    'Windowsへの自動起動登録・解除、管理者昇格、PATH登録・解除'
)
if ($List) {
    [ordered]@{ schemaVersion = 1; features = @($rustFilters.Keys); manualChecks = $manualChecks } |
        ConvertTo-Json -Depth 4
    exit 0
}
if (-not $LogDirectory) {
    Write-Error '対象版のログディレクトリを -LogDirectory で指定してください。' -ErrorAction Continue
    exit 2
}
if ($Feature -eq 'Runtime' -and $RunningProcessId -le 0) {
    Write-Error 'Runtime には非表示で起動した対象プロセスの -RunningProcessId を指定してください。' -ErrorAction Continue
    exit 2
}

$logPath = (New-Item -Path $LogDirectory -ItemType Directory -Force).FullName
$manifest = Join-Path (Resolve-Path -LiteralPath $RustSourceRoot).Path 'src-tauri/Cargo.toml'
$results = [System.Collections.Generic.List[object]]::new()

function Invoke-TestSuite([string]$Name, [string]$Executable, [string[]]$Arguments, [string]$CountPattern) {
    $file = Join-Path $logPath "$Name.log"
    $exitCode = 1
    try {
        & $Executable @Arguments *> $file
        $exitCode = $LASTEXITCODE
    } catch {
        $_ | Out-File -FilePath $file -Append
    }
    $match = [regex]::Match((Get-Content -Raw -LiteralPath $file), $CountPattern)
    $count = if ($match.Success) { [int]$match.Groups[1].Value } else { 0 }
    $results.Add([ordered]@{
        suite = $Name
        passed = ($exitCode -eq 0 -and $count -gt 0)
        testsPassed = $count
        exitCode = $exitCode
        log = $file
    })
}

function Test-RunningApplication([int]$AppProcessId) {
    $file = Join-Path $logPath 'runtime.log'
    try {
        $instance = Get-CimInstance Win32_Process -Filter "ProcessId = $AppProcessId"
        if ($instance.Name -ne 'chuwitchwindow.exe' -or $instance.CommandLine -notmatch '(?:^|\s)--startup(?:\s|$)') {
            throw '指定PIDが非表示起動したChuwitchWindowではありません。'
        }
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        $pipe = [IO.Pipes.NamedPipeClientStream]::new('.', "KotorinToys.$sid.chuwitch-window", [IO.Pipes.PipeDirection]::InOut)
        try {
            $pipe.Connect(1500)
            $writer = [IO.StreamWriter]::new($pipe, [Text.UTF8Encoding]::new($false), 1024, $true)
            $writer.AutoFlush = $true
            $writer.WriteLine((@{ command = 'status'; expectedPid = $AppProcessId } | ConvertTo-Json -Compress))
            $reader = [IO.StreamReader]::new($pipe, [Text.UTF8Encoding]::new($false), $false, 1024, $true)
            $pending = $reader.ReadLineAsync()
            if (-not $pending.Wait(1500) -or -not $pending.Result) { throw '管理窓口が応答しません。' }
            $reply = $pending.Result | ConvertFrom-Json
        } finally {
            $pipe.Dispose()
        }
        if ($reply.protocolVersion -ne 2 -or $reply.pid -ne $AppProcessId -or -not $reply.ready -or $reply.result -ne 'completed') {
            throw '管理窓口が指定PIDの準備完了を返しませんでした。'
        }
        $process = Get-Process -Id $AppProcessId
        if ($process.MainWindowTitle -notin @('', 'com.kotorichun.chuwitchwindow2-siw')) {
            throw '設定ウィンドウが表示されています。'
        }
        @{ pid = $AppProcessId; exe = $instance.ExecutablePath; ready = $true; title = $process.MainWindowTitle } |
            ConvertTo-Json | Set-Content -LiteralPath $file
        $results.Add([ordered]@{ suite = 'runtime'; passed = $true; testsPassed = 1; exitCode = 0; log = $file })
    } catch {
        $_ | Out-File -FilePath $file
        $results.Add([ordered]@{ suite = 'runtime'; passed = $false; testsPassed = 0; exitCode = 1; log = $file })
    }
}

foreach ($filter in $rustFilters[$Feature]) {
    $arguments = @('test', '--locked', '--manifest-path', $manifest, '--lib')
    if ($filter) { $arguments += $filter }
    $name = if ($filter) { 'rust-' + ($filter.TrimEnd(':') -replace '::', '-') } else { 'rust-all' }
    Invoke-TestSuite $name 'cargo.exe' $arguments '(?m)^test result: ok\. (\d+) passed;'
}

if ($Feature -in @('All', 'Frontend', 'Settings')) {
    Push-Location (Join-Path $PSScriptRoot '../..')
    try {
        Invoke-TestSuite 'frontend' 'npm.cmd' @('test') '(?m)^\s*Tests\s+(\d+) passed'
    } finally {
        Pop-Location
    }
}
if ($Feature -in @('All', 'Runtime') -and $RunningProcessId -gt 0) {
    Test-RunningApplication $RunningProcessId
}

$success = $results.Count -gt 0 -and -not ($results | Where-Object { -not $_.passed })
[ordered]@{
    schemaVersion = 1
    feature = $Feature
    passed = $success
    suites = $results.ToArray()
    manualChecks = $manualChecks
} | ConvertTo-Json -Depth 5
if (-not $success) { exit 1 }
exit 0
