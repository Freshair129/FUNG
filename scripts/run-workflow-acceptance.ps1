param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^workflow-[A-Za-z0-9-]{8,80}$')]
    [string]$RunId
)

$ErrorActionPreference = 'Stop'
Import-Module Microsoft.PowerShell.Utility -ErrorAction Stop
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$aiStudio = (Resolve-Path (Join-Path $repo '..\..')).Path
$fixture = Join-Path $aiStudio 'verification\2026-10-15-readiness-fixes\FUNG\fixtures\standalone-audio-48k.wav'
$expectedFixtureSha256 = '493E7CFAD303F6FF3032E0777B811317F48F6C08E74A23DEE779977F674B933F'
$evidenceBase = Join-Path $aiStudio 'verification\fung-workflow-mcp'
$runRoot = Join-Path $evidenceBase $RunId
$acceptanceRoot = Join-Path $runRoot 'acceptance-output'
$targetRoot = Join-Path $aiStudio 'verification\2026-10-15-readiness-fixes\FUNG\cargo-target'
$lockPath = Join-Path $evidenceBase 'workflow-acceptance.lock'
$receiptPath = Join-Path $runRoot 'run-receipt.json'
$logPath = Join-Path $runRoot 'run.log'

function Write-WorkflowProgress {
    param([string]$Stage)
    [Console]::Error.WriteLine('FUNG_WORKFLOW_PROGRESS:' + (@{ stage = $Stage } | ConvertTo-Json -Compress))
}

function Get-FileSha256Hex {
    param([string]$Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '')
    }
    finally {
        $stream.Dispose()
        $algorithm.Dispose()
    }
}

if (-not (Test-Path -LiteralPath $fixture -PathType Leaf)) {
    throw "Pinned synthetic fixture is missing: $fixture"
}
$fixtureHash = Get-FileSha256Hex -Path $fixture
if ($fixtureHash -ne $expectedFixtureSha256) {
    throw "Pinned fixture digest mismatch: expected $expectedFixtureSha256, found $fixtureHash"
}
if (-not (Test-Path -LiteralPath (Join-Path $repo '.venv-whisper\Scripts\python.exe') -PathType Leaf)) {
    throw 'The staged local Whisper runtime is missing. Run scripts/stage_whisper_runtime.ps1 first.'
}
if (Test-Path -LiteralPath $runRoot) {
    throw "Refusing to reuse an existing workflow evidence folder: $runRoot"
}
New-Item -ItemType Directory -Path $evidenceBase -Force | Out-Null

$lock = $null
try {
    try {
        $lock = [System.IO.File]::Open($lockPath, [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
    }
    catch [System.IO.IOException] {
        throw 'Another FUNG workflow acceptance is already using the shared Cargo target; retry after it finishes.'
    }

    New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
    $cargo = (Get-Command cargo -ErrorAction Stop).Source
    $cargoArgs = @(
        'test', '--manifest-path', 'src-tauri\Cargo.toml', '--offline', '--locked',
        '--lib', 'upload_transcribes_with_the_real_runtime', '--', '--ignored', '--nocapture'
    )
    $settings = [ordered]@{
        evidenceKind = 'FUNG functional audio workflow acceptance'
        runId = $RunId
        sourceHead = (& git -C $repo rev-parse HEAD).Trim()
        workingDirectory = $repo
        evidenceRoot = $runRoot
        targetDirectory = $targetRoot
        fixtureId = 'standalone-english-48k'
        fixtureSha256 = $fixtureHash
        transcriptionProfile = 'cpu'
        computeType = 'int8'
        whisperModelProfile = 'large-v3-turbo'
        qualityGate = 'diagnostic_only'
        ollama = 'NOT_TOUCHED'
        gpuInference = 'NOT_RUN'
        command = "cargo $($cargoArgs -join ' ')"
    }
    $receipt = [ordered]@{}
    foreach ($entry in $settings.GetEnumerator()) { $receipt[$entry.Key] = $entry.Value }
    $receipt['status'] = 'RUNNING'
    $receipt['startedAt'] = [DateTimeOffset]::UtcNow.ToString('o')
    $receipt['exitCode'] = $null
    [System.IO.File]::WriteAllText($receiptPath, ($receipt | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
    "COMMAND: $($settings.command)" | Set-Content -LiteralPath $logPath -Encoding UTF8
    "WORKING_DIRECTORY: $repo" | Add-Content -LiteralPath $logPath -Encoding UTF8

    $keys = @(
        'TAURI_CONFIG', 'CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'CARGO_PROFILE_DEV_DEBUG',
        'CARGO_PROFILE_TEST_DEBUG', 'FUNG_UPLOAD_FIXTURE', 'FUNG_UPLOAD_FIXTURE_SHA256',
        'FUNG_UPLOAD_EXPECTED_KEYWORDS', 'FUNG_ACCEPTANCE_ROOT', 'FUNG_TRANSCRIPTION_PROFILE',
        'FUNG_TRANSCRIPTION_COMPUTE_TYPE', 'FUNG_WHISPER_MODEL_PROFILE'
    )
    $previous = @{}
    foreach ($key in $keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
    $started = [Diagnostics.Stopwatch]::StartNew()
    $exitCode = 1
    $runnerError = $null
    try {
        $env:TAURI_CONFIG = '{"bundle":{"resources":[]}}'
        $env:CARGO_TARGET_DIR = $targetRoot
        $env:CARGO_BUILD_JOBS = '1'
        $env:CARGO_PROFILE_DEV_DEBUG = '0'
        $env:CARGO_PROFILE_TEST_DEBUG = '0'
        $env:FUNG_UPLOAD_FIXTURE = $fixture
        $env:FUNG_UPLOAD_FIXTURE_SHA256 = $fixtureHash
        $env:FUNG_UPLOAD_EXPECTED_KEYWORDS = 'local,audio,test,checking,recording,transcription,export,project,reopen,same,words,complete'
        $env:FUNG_ACCEPTANCE_ROOT = $acceptanceRoot
        $env:FUNG_TRANSCRIPTION_PROFILE = 'cpu'
        $env:FUNG_TRANSCRIPTION_COMPUTE_TYPE = 'int8'
        $env:FUNG_WHISPER_MODEL_PROFILE = 'turbo'
        Write-WorkflowProgress 'cargo_acceptance_started'
        Push-Location $repo
        try {
            $previousErrorActionPreference = $ErrorActionPreference
            $ErrorActionPreference = 'Continue'
            try {
                & $cargo @cargoArgs 2>&1 | Tee-Object -FilePath $logPath -Append
                $exitCode = $LASTEXITCODE
            }
            finally {
                $ErrorActionPreference = $previousErrorActionPreference
            }
        }
        finally {
            Pop-Location
        }
    }
    catch {
        $exitCode = 1
        $runnerError = $_.Exception.Message
        [System.IO.File]::AppendAllText($logPath, "`r`nRUNNER_ERROR: $runnerError`r`n", [Text.UTF8Encoding]::new($false))
    }
    finally {
        $started.Stop()
        foreach ($key in $keys) {
            [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process')
        }
    }

    $reportPath = Join-Path $acceptanceRoot 'workflow-result.json'
    $report = if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
        Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    } else { $null }
    if ($null -ne $report) {
        [System.IO.File]::WriteAllText(
            (Join-Path $runRoot 'workflow-result.json'),
            ($report | ConvertTo-Json -Depth 20),
            [Text.UTF8Encoding]::new($false)
        )
    }
    $receipt['durationMs'] = [Math]::Round($started.Elapsed.TotalMilliseconds, 1)
    $receipt['finishedAt'] = [DateTimeOffset]::UtcNow.ToString('o')
    $receipt['exitCode'] = $exitCode
    $receipt['status'] = if ($exitCode -eq 0 -and $report.status -eq 'PASS') { 'PASS' } else { 'FAIL' }
    if ($null -ne $runnerError) { $receipt['error'] = $runnerError }
    $receipt['workflowResult'] = $report
    $receipt['log'] = $logPath
    $receipt['logSha256'] = Get-FileSha256Hex -Path $logPath
    [System.IO.File]::WriteAllText($receiptPath, ($receipt | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    Write-WorkflowProgress 'cargo_acceptance_finished'
    Get-Content -LiteralPath $receiptPath -Raw
    if ($receipt['status'] -ne 'PASS') { exit 1 }
}
finally {
    if ($null -ne $lock) { $lock.Dispose() }
}
