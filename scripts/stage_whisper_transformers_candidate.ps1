[CmdletBinding()]
param(
    [string]$PythonPath,
    [string]$Destination,
    [string]$ModelRevision = 'b751db1e8dbfee6561de22ca99fe070282fcf459',
    [long]$SafetyMarginBytes = 1073741824
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path.TrimEnd('\')
$modelRepo = 'biodatlab/whisper-th-large-combined'
$modelName = 'whisper-th-large-combined'
$expectedModelBytes = 6173655480
$expectedModelSha256 = 'e1e0b5b4c9a89d7d60fb795448c3102e07af87fa73c5fce7c0206c6bd99a7e7b'

if (-not $PythonPath) {
    $PythonPath = Join-Path $repoRoot '.venv-whisper\Scripts\python.exe'
}
if (-not $Destination) {
    $Destination = Join-Path $repoRoot '.venv-whisper-transformers-candidate'
}

$pythonPath = [IO.Path]::GetFullPath($PythonPath)
$destination = [IO.Path]::GetFullPath($Destination)
if (-not $destination.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to stage the candidate runtime outside the repository: $destination"
}
if ($ModelRevision -notmatch '^[0-9a-f]{40}$') {
    throw "ModelRevision must be a 40-character lowercase Hugging Face commit SHA: $ModelRevision"
}
if ($SafetyMarginBytes -lt 0) {
    throw 'SafetyMarginBytes must be zero or greater.'
}
if (-not (Test-Path -LiteralPath $pythonPath -PathType Leaf)) {
    throw "Candidate Python runtime is missing at $pythonPath. Pass -PythonPath explicitly."
}
if (Test-Path -LiteralPath $destination) {
    throw "Refusing to overwrite an existing candidate runtime: $destination"
}

$dependencyCode = @"
import importlib.metadata
import json
import torch
import transformers
from faster_whisper import audio as faster_whisper_audio
print(json.dumps({
    'python': __import__('sys').version.split()[0],
    'torch': torch.__version__,
    'transformers': transformers.__version__,
    'faster_whisper': importlib.metadata.version('faster-whisper'),
    'decoder': faster_whisper_audio.__name__,
}))
"@
$dependencyOutput = & $pythonPath -c $dependencyCode 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "Candidate runtime requires importable torch, transformers, and faster-whisper audio support. Output: $($dependencyOutput -join ' ')"
}
try {
    $dependencyInfo = ($dependencyOutput | Select-Object -Last 1 | ConvertFrom-Json)
} catch {
    throw "Candidate dependency probe returned invalid JSON: $($dependencyOutput -join ' ')"
}

$driveName = ([IO.Path]::GetPathRoot($destination)).Substring(0, 1)
$drive = Get-PSDrive -Name $driveName -ErrorAction Stop
$requiredFreeBytes = $expectedModelBytes + $SafetyMarginBytes
if ($drive.Free -lt $requiredFreeBytes) {
    throw "Insufficient free space on $driveName`: required at least $requiredFreeBytes bytes for the pinned checkpoint plus safety margin; found $($drive.Free). No artifact was created."
}

$destinationParent = Split-Path -Parent $destination
$stagingRoot = Join-Path $destinationParent ('.whisper-thai-candidate-stage-' + [Guid]::NewGuid().ToString('N'))
$modelDir = Join-Path $stagingRoot "models\$modelName"
$patterns = @(
    'README.md',
    'added_tokens.json',
    'config.json',
    'generation_config.json',
    'merges.txt',
    'normalizer.json',
    'preprocessor_config.json',
    'pytorch_model.bin',
    'special_tokens_map.json',
    'tokenizer_config.json',
    'vocab.json'
)
$allowPatternsJson = $patterns | ConvertTo-Json -Compress

try {
    New-Item -ItemType Directory -Path $modelDir -Force | Out-Null
    $downloadCode = @"
from huggingface_hub import snapshot_download
snapshot_download(
    repo_id='$modelRepo',
    revision='$ModelRevision',
    local_dir=r'$modelDir',
    allow_patterns=$allowPatternsJson,
)
"@
    $downloadOutput = & $pythonPath -c $downloadCode 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned Transformers model download failed with exit code ${LASTEXITCODE}: $($downloadOutput -join ' ')"
    }

    $checkpoint = Join-Path $modelDir 'pytorch_model.bin'
    if (-not (Test-Path -LiteralPath $checkpoint -PathType Leaf)) {
        throw "Pinned checkpoint is missing after download: $checkpoint"
    }
    $checkpointInfo = Get-Item -LiteralPath $checkpoint
    if ($checkpointInfo.Length -ne $expectedModelBytes) {
        throw "Pinned checkpoint size mismatch: expected $expectedModelBytes, got $($checkpointInfo.Length)"
    }
    $checkpointHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $checkpoint).Hash.ToLowerInvariant()
    if ($checkpointHash -ne $expectedModelSha256) {
        throw "Pinned checkpoint SHA-256 mismatch: expected $expectedModelSha256, got $checkpointHash"
    }

    $requiredFiles = @('config.json', 'generation_config.json', 'preprocessor_config.json', 'pytorch_model.bin', 'tokenizer_config.json', 'vocab.json')
    foreach ($requiredFile in $requiredFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $modelDir $requiredFile) -PathType Leaf)) {
            throw "Candidate model is incomplete; required file is missing: $requiredFile"
        }
    }

    $files = Get-ChildItem -LiteralPath $stagingRoot -Recurse -File | ForEach-Object {
        [ordered]@{
            path = $_.FullName.Substring($stagingRoot.Length).TrimStart('\').Replace('\', '/')
            bytes = $_.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
        }
    }
    $manifest = [ordered]@{
        schemaVersion = 1
        generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
        backend = 'transformers'
        candidateProfile = 'thai-large-candidate'
        model = [ordered]@{
            name = $modelName
            repository = $modelRepo
            revision = $ModelRevision
            license = 'Apache-2.0'
            sourceBase = 'openai/whisper-large-v2'
        }
        dependencies = $dependencyInfo
        selectedFiles = $patterns
        files = $files
    }
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $stagingRoot 'manifest.json') -Encoding utf8

    Move-Item -LiteralPath $stagingRoot -Destination $destination
    $stagingRoot = $null
    Write-Host "Staged opt-in FUNG Transformers candidate at $destination"
    Write-Host "Model: $modelRepo ($ModelRevision)"
    Write-Host "Checkpoint: $expectedModelBytes bytes / $expectedModelSha256"
} finally {
    if ($stagingRoot -and (Test-Path -LiteralPath $stagingRoot)) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force
        Write-Host 'Removed incomplete candidate staging directory.'
    }
}
