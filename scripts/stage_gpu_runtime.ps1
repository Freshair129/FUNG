# Stages the CUDA 12 / cuDNN 9 DLLs the GPU transcription profile needs into
# runtime\cuda12in and writes runtime\manifest.json (SHA-256 per file).
#
# -CudaSource must be a single directory holding all 11 DLLs. The easiest
# source that needs no CUDA Toolkit or Torch install is NVIDIA's own pip
# wheels (verified 2026-09-16, ~1.3 GB download):
#
#   pip download nvidia-cublas-cu12 nvidia-cudnn-cu12 nvidia-cuda-runtime-cu12 `
#       --no-deps --only-binary=:all: --platform win_amd64 --python-version 3.11 -d wheels
#   # unzip each wheel's nvidia\*in\*.dll into one folder, then:
#   .\scripts\stage_gpu_runtime.ps1 -CudaSource <that folder>
#
# The default -CudaSource is a historical Torch install and will usually not
# exist; pass the directory explicitly.
[CmdletBinding()]
param(
    [string]$CudaSource = 'D:\G-Music\backend\.venv\Lib\site-packages\torch\lib',
    [switch]$Clean
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $repoRoot 'runtime\cuda12\bin'
$manifestPath = Join-Path $repoRoot 'runtime\manifest.json'
$requiredFiles = @(
    'cudart64_12.dll',
    'cublas64_12.dll',
    'cublasLt64_12.dll',
    'cudnn64_9.dll',
    'cudnn_adv64_9.dll',
    'cudnn_cnn64_9.dll',
    'cudnn_engines_precompiled64_9.dll',
    'cudnn_engines_runtime_compiled64_9.dll',
    'cudnn_graph64_9.dll',
    'cudnn_heuristic64_9.dll',
    'cudnn_ops64_9.dll'
)

if ($Clean) {
    if (Test-Path -LiteralPath $destination) {
        Remove-Item -LiteralPath $destination -Recurse -Force
    }
    if (Test-Path -LiteralPath $manifestPath) {
        Remove-Item -LiteralPath $manifestPath -Force
    }
    Write-Host 'Removed staged FUNG CUDA runtime.'
    exit 0
}

if (-not (Test-Path -LiteralPath $CudaSource -PathType Container)) {
    throw "CUDA source directory was not found: $CudaSource"
}

$missing = $requiredFiles | Where-Object { -not (Test-Path -LiteralPath (Join-Path $CudaSource $_) -PathType Leaf) }
if ($missing) {
    throw "CUDA source directory is incomplete. Missing: $($missing -join ', ')"
}

New-Item -ItemType Directory -Path $destination -Force | Out-Null
$files = foreach ($name in $requiredFiles) {
    $source = Join-Path $CudaSource $name
    $target = Join-Path $destination $name
    Copy-Item -LiteralPath $source -Destination $target -Force
    $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $target
    [ordered]@{
        name = $name
        sha256 = $hash.Hash.ToLowerInvariant()
        bytes = (Get-Item -LiteralPath $target).Length
    }
}

$manifest = [ordered]@{
    schemaVersion = 1
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    source = 'NVIDIA CUDA 12 / cuDNN 9 files staged from an explicitly supplied local CUDA-compatible distribution'
    redistribution = 'Release owner must verify NVIDIA redistribution terms before publishing this bundle.'
    files = $files
}
New-Item -ItemType Directory -Path (Split-Path -Parent $manifestPath) -Force | Out-Null
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding utf8
Write-Host "Staged $($requiredFiles.Count) CUDA/cuDNN DLLs to $destination"
