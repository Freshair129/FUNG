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
