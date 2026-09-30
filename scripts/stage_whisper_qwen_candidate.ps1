[CmdletBinding()]
param(
    [string]$Destination,
    [long]$SafetyMarginBytes = 10737418240
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path.TrimEnd('\')
$pythonVersion = '3.12.10'
$pythonUrl = "https://www.python.org/ftp/python/$pythonVersion/python-$pythonVersion-embed-amd64.zip"
$pythonSha256 = '4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3'
$asrRepo = 'Qwen/Qwen3-ASR-1.7B-hf'
$asrRevision = 'bcd2b5b7f32b480ab5790554cfa8347f246a14f3'
$asrName = 'qwen3-asr-1.7b'
$asrBytes = 4076193080
$asrSha256 = '2db53c7d81bd9b8cbc6a074e89be2c968a0d373fb4ee68bb1b1e14f7042dfee1'
$alignerRepo = 'wannaphong/wav2vec2-large-xlsr-53-th-cv8-newmm'
$alignerRevision = '18381b4cfbe8b2e7462827f2c6dea681a31ef5b9'
$alignerName = 'thai-wav2vec2-ctc'
$alignerBytes = 1262102632
$alignerSha256 = 'f0135130a25f0f16a59cc376f886f9e54e0c1736f1b85c4c01e93b9f4cc4b090'

if (-not $Destination) {
    $Destination = Join-Path $repoRoot '.venv-whisper-qwen-candidate'
}
$destination = [IO.Path]::GetFullPath($Destination)
if (-not $destination.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to stage the Qwen candidate runtime outside the repository: $destination"
}
if ($SafetyMarginBytes -lt 0) {
    throw 'SafetyMarginBytes must be zero or greater.'
}
if (Test-Path -LiteralPath $destination) {
    throw "Refusing to overwrite an existing Qwen candidate runtime: $destination"
}

$uv = Get-Command uv -ErrorAction SilentlyContinue
if (-not $uv) {
    throw 'The uv CLI is required to install the Qwen candidate dependencies from the hash-locked file.'
}
$requirements = Join-Path $PSScriptRoot 'qwen-candidate-requirements.txt'
if (-not (Test-Path -LiteralPath $requirements -PathType Leaf)) {
    throw "Qwen candidate dependency lock is missing: $requirements"
}
$driveName = ([IO.Path]::GetPathRoot($destination)).Substring(0, 1)
$drive = Get-PSDrive -Name $driveName -ErrorAction Stop
$requiredFreeBytes = $asrBytes + $alignerBytes + $SafetyMarginBytes
if ($drive.Free -lt $requiredFreeBytes) {
    throw "Insufficient free space on $driveName`: required at least $requiredFreeBytes bytes for the pinned Qwen runtime; found $($drive.Free). No artifact was created."
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

$destinationParent = Split-Path -Parent $destination
$stagingRoot = Join-Path $destinationParent ('.whisper-qwen-candidate-stage-' + [Guid]::NewGuid().ToString('N'))
$resolvedStagingParent = [IO.Path]::GetFullPath($destinationParent).TrimEnd('\')
if (-not $resolvedStagingParent.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase) -and
    $resolvedStagingParent -ne $repoRoot) {
    throw "Refusing to create a Qwen staging directory outside repository: $resolvedStagingParent"
}
$scriptsDir = Join-Path $stagingRoot 'Scripts'
$sitePackages = Join-Path $stagingRoot 'Lib\site-packages'
$asrDir = Join-Path $stagingRoot "models\$asrName"
$alignerDir = Join-Path $stagingRoot "models\$alignerName"
$pythonPath = Join-Path $scriptsDir 'python.exe'
$pythonSourceFile = Join-Path $scriptsDir 'python312._pth'
$asrPatterns = @(
    'README.md',
    'chat_template.jinja',
    'config.json',
    'generation_config.json',
    'model.safetensors',
    'processor_config.json',
    'tokenizer.json',
    'tokenizer_config.json'
)
$alignerPatterns = @(
    'README.md',
    'added_tokens.json',
    'config.json',
    'model.safetensors',
    'preprocessor_config.json',
    'special_tokens_map.json',
    'tokenizer_config.json',
    'vocab.json'
)

try {
    New-Item -ItemType Directory -Path $scriptsDir, $sitePackages, $asrDir, $alignerDir -Force | Out-Null
    Expand-Archive -LiteralPath $pythonArchive -DestinationPath $scriptsDir -Force
    @(
        'python312.zip'
        '.'
        '..\Lib\site-packages'
        'import site'
    ) | Set-Content -LiteralPath $pythonSourceFile -Encoding ascii

    & $uv.Source pip install --python $pythonPath --target $sitePackages --only-binary=:all: --require-hashes --index-strategy unsafe-best-match --requirement $requirements
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned Qwen candidate dependency install failed with exit code $LASTEXITCODE"
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
import pythainlp
print(json.dumps({
    'python': sys.version.split()[0],
    'torch': torch.__version__,
    'transformers': transformers.__version__,
    'accelerate': accelerate.__version__,
    'fasterWhisper': importlib.metadata.version('faster-whisper'),
    'av': av.__version__,
    'pythainlp': pythainlp.__version__,
    'cudaAvailable': torch.cuda.is_available(),
    'cudaVersion': torch.version.cuda,
    'packages': {
        distribution.metadata['Name']: distribution.version
        for distribution in importlib.metadata.distributions()
        if distribution.metadata.get('Name')
    },
}))
"@
    $dependencyOutput = & $pythonPath -c $dependencyCode 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Qwen candidate runtime import probe failed: $($dependencyOutput -join ' ')"
    }
    try {
        $dependencyInfo = ($dependencyOutput | Select-Object -Last 1 | ConvertFrom-Json)
    } catch {
        throw "Qwen candidate runtime probe returned invalid JSON: $($dependencyOutput -join ' ')"
    }
    if ($dependencyInfo.python -ne $pythonVersion -or
        $dependencyInfo.torch -ne '2.14.0+cu130' -or
        $dependencyInfo.transformers -ne '5.16.1' -or
        $dependencyInfo.accelerate -ne '1.10.1' -or
        $dependencyInfo.fasterWhisper -ne '1.2.1' -or
        $dependencyInfo.av -ne '18.1.0' -or
        $dependencyInfo.pythainlp -ne '5.3.8' -or
        $dependencyInfo.cudaVersion -ne '13.0') {
        throw "Qwen candidate runtime versions do not match the approved lock: $($dependencyOutput -join ' ')"
    }

    $downloadAsr = @"
from huggingface_hub import snapshot_download
snapshot_download(repo_id='$asrRepo', revision='$asrRevision', local_dir=r'$asrDir', allow_patterns=$(ConvertTo-Json -Compress -InputObject $asrPatterns))
"@
    $asrOutput = & $pythonPath -c $downloadAsr 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned Qwen ASR model staging failed with exit code ${LASTEXITCODE}: $($asrOutput -join ' ')"
    }
    $downloadAligner = @"
from huggingface_hub import snapshot_download
snapshot_download(repo_id='$alignerRepo', revision='$alignerRevision', local_dir=r'$alignerDir', allow_patterns=$(ConvertTo-Json -Compress -InputObject $alignerPatterns))
"@
    $alignerOutput = & $pythonPath -c $downloadAligner 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned Thai CTC model staging failed with exit code ${LASTEXITCODE}: $($alignerOutput -join ' ')"
    }

    foreach ($model in @(
        @{ Directory = $asrDir; Name = 'ASR'; Bytes = $asrBytes; Sha256 = $asrSha256 },
        @{ Directory = $alignerDir; Name = 'Thai CTC'; Bytes = $alignerBytes; Sha256 = $alignerSha256 }
    )) {
        $checkpoint = Join-Path $model.Directory 'model.safetensors'
        if (-not (Test-Path -LiteralPath $checkpoint -PathType Leaf)) {
            throw "$($model.Name) checkpoint is missing after staging: $checkpoint"
        }
        $checkpointInfo = Get-Item -LiteralPath $checkpoint
        if ($checkpointInfo.Length -ne $model.Bytes) {
            throw "$($model.Name) checkpoint size mismatch: expected $($model.Bytes), got $($checkpointInfo.Length)"
        }
        $checkpointHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $checkpoint).Hash.ToLowerInvariant()
        if ($checkpointHash -ne $model.Sha256) {
            throw "$($model.Name) checkpoint SHA-256 mismatch: expected $($model.Sha256), got $checkpointHash"
        }
    }
    foreach ($requiredFile in $asrPatterns) {
        if (-not (Test-Path -LiteralPath (Join-Path $asrDir $requiredFile) -PathType Leaf)) {
            throw "Pinned Qwen ASR model is incomplete; missing $requiredFile"
        }
    }
    foreach ($requiredFile in $alignerPatterns) {
        if (-not (Test-Path -LiteralPath (Join-Path $alignerDir $requiredFile) -PathType Leaf)) {
            throw "Pinned Thai CTC model is incomplete; missing $requiredFile"
        }
    }

    $modelFiles = @()
    foreach ($model in @(
        @{ Name = $asrName; Directory = $asrDir; Patterns = $asrPatterns },
        @{ Name = $alignerName; Directory = $alignerDir; Patterns = $alignerPatterns }
    )) {
        $files = foreach ($pattern in $model.Patterns) {
            $file = Get-Item -LiteralPath (Join-Path $model.Directory $pattern)
            [ordered]@{
                path = "$($model.Name)/$($file.Name)"
                bytes = $file.Length
                sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $file.FullName).Hash.ToLowerInvariant()
            }
        }
        $modelFiles += $files
    }

    $manifest = [ordered]@{
        schemaVersion = 1
        generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
        backend = 'transformers'
        candidateProfile = 'qwen-thai-candidate'
        python = [ordered]@{
            version = $pythonVersion
            source = $pythonUrl
            sha256 = $pythonSha256
            interpreter = 'Scripts/python.exe'
        }
        audio = [ordered]@{
            preprocessingProfile = 'raw'
        }
        dependencies = [ordered]@{
            lockfile = 'scripts/qwen-candidate-requirements.txt'
            lockfileSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $requirements).Hash.ToLowerInvariant()
            torch = $dependencyInfo.torch
            transformers = $dependencyInfo.transformers
            accelerate = $dependencyInfo.accelerate
            fasterWhisper = $dependencyInfo.fasterWhisper
            av = $dependencyInfo.av
            pythainlp = $dependencyInfo.pythainlp
            cudaVersion = $dependencyInfo.cudaVersion
            runtimePackages = $dependencyInfo.packages
        }
        models = [ordered]@{
            asr = [ordered]@{
                name = $asrName
                repository = $asrRepo
                revision = $asrRevision
                license = 'Apache-2.0'
                checkpoint = [ordered]@{ path = "$asrName/model.safetensors"; bytes = $asrBytes; sha256 = $asrSha256 }
            }
            aligner = [ordered]@{
                name = $alignerName
                repository = $alignerRepo
                revision = $alignerRevision
                license = 'Apache-2.0'
                checkpoint = [ordered]@{ path = "$alignerName/model.safetensors"; bytes = $alignerBytes; sha256 = $alignerSha256 }
            }
        }
        files = $modelFiles
        qualification = [ordered]@{
            ctcFeasibility = 'pending-fung-worker-pilot'
            pilotQuality = 'pending'
            microphoneSpotChecks = 'pending-human-review'
            detailedRouting = $false
        }
    }
    $manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $stagingRoot 'manifest.json') -Encoding utf8

    if (Test-Path -LiteralPath $destination) {
        throw "Qwen candidate runtime appeared during staging; refusing to overwrite: $destination"
    }
    Move-Item -LiteralPath $stagingRoot -Destination $destination
    $stagingRoot = $null
    Write-Host "Staged isolated FUNG Qwen candidate at $destination"
    Write-Host "Python: $pythonVersion; Torch: $($dependencyInfo.torch); Transformers: $($dependencyInfo.transformers); CUDA: $($dependencyInfo.cudaVersion)"
    Write-Host "ASR: $asrRepo ($asrRevision) / $asrBytes bytes / $asrSha256"
    Write-Host "Aligner: $alignerRepo ($alignerRevision) / $alignerBytes bytes / $alignerSha256"
    Write-Host 'CTC feasibility and human microphone spot checks remain qualification gates; Detailed routing is disabled.'
} finally {
    if ($stagingRoot -and (Test-Path -LiteralPath $stagingRoot)) {
        $resolvedStagingRoot = [IO.Path]::GetFullPath($stagingRoot)
        if (-not $resolvedStagingRoot.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove Qwen staging directory outside repository: $resolvedStagingRoot"
        }
        Remove-Item -LiteralPath $resolvedStagingRoot -Recurse -Force
        Write-Host 'Removed incomplete Qwen candidate staging directory.'
    }
}
