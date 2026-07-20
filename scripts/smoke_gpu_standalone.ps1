[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$BundleRoot,
    [Parameter(Mandatory)]
    [string]$AudioFixture,
    [ValidateSet('gpu', 'cpu')]
    [string]$Profile = 'gpu'
)

$ErrorActionPreference = 'Stop'
$bundleRoot = (Resolve-Path -LiteralPath $BundleRoot).Path
$audioFixture = (Resolve-Path -LiteralPath $AudioFixture).Path
$python = Join-Path $bundleRoot '.venv-whisper\Scripts\python.exe'
$worker = Join-Path $bundleRoot 'scripts\transcribe.py'
$cudaBin = Join-Path $bundleRoot 'runtime\cuda12\bin'
$required = @('cudart64_12.dll', 'cublas64_12.dll', 'cublasLt64_12.dll', 'cudnn64_9.dll')

if ((@($required | Where-Object { -not (Test-Path -LiteralPath (Join-Path $cudaBin $_) -PathType Leaf) })).Count -gt 0) {
    throw 'FUNG bundle is missing required CUDA runtime DLLs.'
}
if (-not (Test-Path -LiteralPath $python -PathType Leaf) -or -not (Test-Path -LiteralPath $worker -PathType Leaf)) {
    throw 'FUNG bundle is missing the packaged Python worker.'
}

$cleanPath = (($env:PATH -split ';' | Where-Object {
    $_ -and $_ -notmatch '(?i)g-music|torch|cuda\\v\d+\.\d+\\bin'
}) -join ';')
$env:PATH = "$cudaBin;$cleanPath"
$env:FUNG_TRANSCRIPTION_PROFILE = $Profile

& $python $worker $audioFixture --profile $Profile
if ($LASTEXITCODE -ne 0) {
    throw "Standalone GPU smoke test failed with exit code $LASTEXITCODE."
}
