$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Push-Location $workspace
$trace = [IO.Path]::GetTempFileName()
$previousTrace = $env:ZGUI_LATENCY
$viewer = $null
try {
    & cargo build --locked --release -p enlumi
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $env:ZGUI_LATENCY = $trace
    $viewer = Start-Process -FilePath (Join-Path $workspace 'target/release/enlumi.exe') -WorkingDirectory $workspace -WindowStyle Normal -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 250
        $viewer.Refresh()
        if ($viewer.HasExited) { throw "Startup exited: $($viewer.ExitCode)" }
        $ready = $viewer.MainWindowTitle -eq 'Enlumi - PX4 log workbench' -and $viewer.Responding
    } until ($ready -or [DateTime]::UtcNow -ge $deadline)
    if (-not $ready) { throw 'No responsive Enlumi window within 30 seconds' }
    Start-Sleep -Seconds 1
    if (-not $viewer.CloseMainWindow() -or -not $viewer.WaitForExit(10000)) { throw 'GUI failed clean close' }
    if ($viewer.ExitCode -ne 0) { throw "GUI exited: $($viewer.ExitCode)" }
    $frames = @(Get-Content -LiteralPath $trace | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object { $_.stage -eq 'f.end' -and $_.note -match '^Presented' })
    if ($frames.Count -lt 2) { throw 'GUI did not present two frames' }
    Write-Output "Startup passed: responsive GUI, $($frames.Count) presented frames, clean exit."
} finally {
    if ($null -ne $viewer -and -not $viewer.HasExited) { $viewer.Kill(); $viewer.WaitForExit() }
    $env:ZGUI_LATENCY = $previousTrace
    Remove-Item -LiteralPath $trace
    Pop-Location
}
