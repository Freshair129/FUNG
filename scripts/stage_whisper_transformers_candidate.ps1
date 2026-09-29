[CmdletBinding()]
param(
    [string]$Destination,
    [string]$ModelRevision = 'b751db1e8dbfee6561de22ca99fe070282fcf459',
    [long]$SafetyMarginBytes = 4294967296
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path.TrimEnd('\')
$pythonVersion = '3.11.9'
$pythonUrl = "https://www.python.org/ftp/python/$pythonVersion/python-$pythonVersion-embed-amd64.zip"
$pythonSha256 = '009d6bf7e3b2ddca3d784fa09f90fe54336d5b60f0e0f305c37f400bf83cfd3b'
$modelRepo = 'biodatlab/whisper-th-large-combined'
$modelName = 'whisper-th-large-combined'
$expectedModelBytes = 6173655480
$expectedModelSha256 = 'e1e0b5b4c9a89d7d60fb795448c3102e07af87fa73c5fce7c0206c6bd99a7e7b'

if (-not $Destination) {
    $Destination = Join-Path $repoRoot '.venv-whisper-transformers-candidate'
}
$destination = [IO.Path]::GetFullPath($Destination)
if (-not $destination.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to stage the candidate runtime outside the repository: $destination"
}
if ($ModelRevision -ne 'b751db1e8dbfee6561de22ca99fe070282fcf459') {
    throw 'Only the approved candidate model revision may be staged.'
}
if ($SafetyMarginBytes -lt 0) {
    throw 'SafetyMarginBytes must be zero or greater.'
}
if (Test-Path -LiteralPath $destination) {
    throw "Refusing to overwrite an existing candidate runtime: $destination"
}

$uv = Get-Command uv -ErrorAction SilentlyContinue
if (-not $uv) {
    throw 'The uv CLI is required to install the candidate-only, hash-locked dependencies.'
}
$requirements = Join-Path $PSScriptRoot 'transformers-candidate-requirements.txt'
if (-not (Test-Path -LiteralPath $requirements -PathType Leaf)) {
    throw "Candidate dependency lock is missing: $requirements"
}

$destinationParent = Split-Path -Parent $destination
$driveName = ([IO.Path]::GetPathRoot($destination)).Substring(0, 1)
$drive = Get-PSDrive -Name $driveName -ErrorAction Stop
$requiredFreeBytes = $expectedModelBytes + $SafetyMarginBytes
if ($drive.Free -lt $requiredFreeBytes) {
    throw "Insufficient free space on $driveName`: required at least $requiredFreeBytes bytes for the pinned candidate runtime; found $($drive.Free). No artifact was created."
}

$cacheRoot = Join-Path $repoRoot '.runtime-cache'
$pythonArchive = Join-Path $cacheRoot "python-$pythonVersion-embed-amd64.zip"
if (-not (Test-Path -LiteralPath $pythonArchive -PathType Leaf)) {
    New-Item -ItemType Directory -Path $cacheRoot -Force | Out-Null
    Invoke-WebRequest -Uri $pythonUrl -OutFile $pythonArchive
}
$actualPythonHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $pythonArchive).Hash.ToLowerInvariant()
if ($actualPythonHash -ne $pythonSha256) {
    throw "Python archive SHA-256 mismatch: expected $pythonSha256, got $actualPythonHash"
}

$stagingRoot = Join-Path $destinationParent ('.whisper-thai-candidate-stage-' + [Guid]::NewGuid().ToString('N'))
$resolvedStagingParent = [IO.Path]::GetFullPath($destinationParent).TrimEnd('\')
if (-not $resolvedStagingParent.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase) -and
    $resolvedStagingParent -ne $repoRoot) {
    throw "Refusing to create a staging directory outside repository: $resolvedStagingParent"
}
$scriptsDir = Join-Path $stagingRoot 'Scripts'
$sitePackages = Join-Path $stagingRoot 'Lib\site-packages'
$modelDir = Join-Path $stagingRoot "models\$modelName"
$pythonPath = Join-Path $scriptsDir 'python.exe'
$pythonSourceFile = Join-Path $scriptsDir 'python311._pth'
$modelPatterns = @(
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

try {
    New-Item -ItemType Directory -Path $scriptsDir, $sitePackages, $modelDir -Force | Out-Null
    Expand-Archive -LiteralPath $pythonArchive -DestinationPath $scriptsDir -Force
    @(
        'python311.zip'
        '.'
        '..\Lib\site-packages'
        'import site'
    ) | Set-Content -LiteralPath $pythonSourceFile -Encoding ascii

    & $uv.Source pip install --python $pythonPath --target $sitePackages --only-binary=:all: --require-hashes --index-strategy unsafe-best-match --requirement $requirements
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned candidate dependency install failed with exit code $LASTEXITCODE"
    }

    $dependencyCode = @"
import importlib.metadata
import json
import sys
import torch
import transformers
import accelerate
import faster_whisper.audio
import av
print(json.dumps({
    'python': sys.version.split()[0],
    'torch': torch.__version__,
    'transformers': transformers.__version__,
    'accelerate': accelerate.__version__,
    'fasterWhisper': importlib.metadata.version('faster-whisper'),
    'av': av.__version__,
    'packages': {
        distribution.metadata['Name']: distribution.version
        for distribution in importlib.metadata.distributions()
        if distribution.metadata.get('Name')
    },
}))
"@
    $dependencyOutput = & $pythonPath -c $dependencyCode 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Candidate runtime import probe failed: $($dependencyOutput -join ' ')"
    }
    try {
        $dependencyInfo = ($dependencyOutput | Select-Object -Last 1 | ConvertFrom-Json)
    } catch {
        throw "Candidate runtime probe returned invalid JSON: $($dependencyOutput -join ' ')"
    }
    if ($dependencyInfo.python -ne $pythonVersion -or
        $dependencyInfo.torch -ne '2.14.0+cpu' -or
        $dependencyInfo.transformers -ne '4.57.1' -or
        $dependencyInfo.accelerate -ne '1.10.1' -or
        $dependencyInfo.fasterWhisper -ne '1.2.1' -or
        $dependencyInfo.av -ne '18.1.0') {
        throw "Candidate runtime versions do not match the approved lock: $($dependencyOutput -join ' ')"
    }

    $allowPatternsJson = $modelPatterns | ConvertTo-Json -Compress
    $modelPathForPython = $modelDir
    $downloadCode = @"
from huggingface_hub import snapshot_download
snapshot_download(
    repo_id='$modelRepo',
    revision='$ModelRevision',
    local_dir=r'$modelPathForPython',
    allow_patterns=$allowPatternsJson,
)
"@
    $downloadOutput = & $pythonPath -c $downloadCode 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned Transformers model staging failed with exit code ${LASTEXITCODE}: $($downloadOutput -join ' ')"
    }

    $checkpoint = Join-Path $modelDir 'pytorch_model.bin'
    if (-not (Test-Path -LiteralPath $checkpoint -PathType Leaf)) {
        throw "Pinned checkpoint is missing after staging: $checkpoint"
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

    $modelFiles = Get-ChildItem -LiteralPath $modelDir -File | ForEach-Object {
        [ordered]@{
            path = $_.Name
            bytes = $_.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
        }
    }
    $manifest = [ordered]@{
        schemaVersion = 2
        generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
        backend = 'transformers'
        candidateProfile = 'thai-large-candidate'
        python = [ordered]@{
            version = $pythonVersion
            source = $pythonUrl
            sha256 = $pythonSha256
            interpreter = 'Scripts/python.exe'
        }
        dependencies = [ordered]@{
            lockfile = 'scripts/transformers-candidate-requirements.txt'
            lockfileSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $requirements).Hash.ToLowerInvariant()
            torch = $dependencyInfo.torch
            transformers = $dependencyInfo.transformers
            accelerate = $dependencyInfo.accelerate
            fasterWhisper = $dependencyInfo.fasterWhisper
            av = $dependencyInfo.av
            runtimePackages = $dependencyInfo.packages
        }
        model = [ordered]@{
            name = $modelName
            repository = $modelRepo
            revision = $ModelRevision
            license = 'Apache-2.0'
            sourceBase = 'openai/whisper-large-v2'
            checkpoint = [ordered]@{
                path = 'pytorch_model.bin'
                bytes = $checkpointInfo.Length
                sha256 = $checkpointHash
            }
            files = $modelFiles
        }
        selectedFiles = $modelPatterns
    }
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $stagingRoot 'manifest.json') -Encoding utf8

    if (Test-Path -LiteralPath $destination) {
        throw "Candidate runtime appeared during staging; refusing to overwrite: $destination"
    }
    Move-Item -LiteralPath $stagingRoot -Destination $destination
    $stagingRoot = $null
    Write-Host "Staged isolated FUNG Transformers candidate at $destination"
    Write-Host "Model: $modelRepo ($ModelRevision)"
    Write-Host "Python: $pythonVersion; Torch: $($dependencyInfo.torch); Transformers: $($dependencyInfo.transformers)"
    Write-Host "Checkpoint: $expectedModelBytes bytes / $expectedModelSha256"
} finally {
    if ($stagingRoot -and (Test-Path -LiteralPath $stagingRoot)) {
        $resolvedStagingRoot = [IO.Path]::GetFullPath($stagingRoot)
        if (-not $resolvedStagingRoot.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove staging directory outside repository: $resolvedStagingRoot"
        }
        Remove-Item -LiteralPath $resolvedStagingRoot -Recurse -Force
        Write-Host 'Removed incomplete candidate staging directory.'
    }
}
