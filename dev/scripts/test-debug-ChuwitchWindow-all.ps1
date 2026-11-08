[CmdletBinding()]
param(
    [ValidateSet('All', 'Drag', 'MonitorMovement', 'Arrange', 'Rules', 'Settings', 'Pin', 'Hotkeys', 'Desktop', 'Platform', 'Frontend', 'Runtime')]
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
    Desktop = @('desktop_tests::', 'tray::', 'hotplug::')
    Platform = @('admin::')
    Frontend = @()
    Runtime = @()
}
$isolation = [ordered]@{
    Drag = 'マウスフックに続く入力処理へ試験イベントを渡し、専用の実ウィンドウで受信・座標・サイズ・前後関係を検証'
    MonitorMovement = '仮想のモニター矩形と専用の実ウィンドウを使用'
    Hotkeys = 'OSのホットキーイベントをディスパッチ入口に渡す'
    Desktop = '隠し通知窓へのWM_DISPLAYCHANGEと設定画面・トレイの試験用ウィンドウ状態を使用'
    Platform = '隔離レジストリキーとタスクスケジューラ・昇格要求の代替実装を使用'
}
$requiredCases = [ordered]@{
    Drag = @('middle_input_from_title_moves_test_group_and_client_click_reaches_window', 'cross_monitor_drop_moves_fixed_window_without_resizing_it', 'raises_all_group_members_above_another_window_without_moving_them')
    MonitorMovement = @('move_actions_apply_coordinates_and_undo_restores_test_windows', 'plans_placement_for_two_sample_displays_without_touching_windows')
    Arrange = @('all_arrange_modes_apply_expected_rectangles_to_test_windows', 'plans_grid_on_selected_sample_display_without_touching_windows')
    Hotkeys = @('isolated_key_events_dispatch_each_action_and_skip_release_and_recording')
    Desktop = @('settings_window_is_created_and_reused_by_the_window_controller', 'settings_minimize_and_arrange_focus_loss_request_window_close', 'tray_input_opens_settings_and_routes_restart_and_quit', 'display_change_message_notifies_the_application_without_changing_monitors')
    Platform = @('startup_registration_and_removal_read_back_an_isolated_scheduler', 'path_registration_round_trips_in_a_separate_registry_key', 'elevation_request_uses_runas_and_recognizes_failure_without_elevating')
}
if ($List) {
    [ordered]@{ schemaVersion = 1; features = @($rustFilters.Keys); isolation = $isolation; manualChecks = @() } |
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
$verifiedCases = [System.Collections.Generic.HashSet[string]]::new()

function Invoke-TestSuite([string]$Name, [string]$Executable, [string[]]$Arguments, [string]$CountPattern) {
    $file = Join-Path $logPath "$Name.log"
    $exitCode = 1
    try {
        & $Executable @Arguments *> $file
        $exitCode = $LASTEXITCODE
    } catch {
        $_ | Out-File -FilePath $file -Append
    }
    $output = Get-Content -Raw -LiteralPath $file
    $match = [regex]::Match($output, $CountPattern)
    foreach ($case in [regex]::Matches($output, '(?m)^test (?:\w+::)*(\w+) \.\.\. ok\r?$')) {
        [void]$verifiedCases.Add($case.Groups[1].Value)
    }
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

$expected = if ($Feature -eq 'All') { $requiredCases.Values | ForEach-Object { $_ } } else { $requiredCases[$Feature] }
$missing = @($expected | Where-Object { $_ -and -not $verifiedCases.Contains($_) })
$success = $results.Count -gt 0 -and $missing.Count -eq 0 -and -not ($results | Where-Object { -not $_.passed })
[ordered]@{
    schemaVersion = 1
    feature = $Feature
    passed = $success
    suites = $results.ToArray()
    missingCases = $missing
    isolation = $isolation
    manualChecks = @()
} | ConvertTo-Json -Depth 5
if (-not $success) { exit 1 }
exit 0
