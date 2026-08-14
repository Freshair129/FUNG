[CmdletBinding()]
param(
    [string]$PythonVersion = '3.11.9',
    [string]$FasterWhisperVersion = '1.2.1',
    [string]$Model = 'small',
    [string]$HostPython,
    [switch]$Clean
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $repoRoot '.venv-whisper'
$cacheRoot = Join-Path $repoRoot '.runtime-cache'
$pythonUrl = "https://www.python.org/ftp/python/$PythonVersion/python-$PythonVersion-embed-amd64.zip"
$pythonSha256 = '009d6bf7e3b2ddca3d784fa09f90fe54336d5b60f0e0f305c37f400bf83cfd3b'
$modelRepo = 'Systran/faster-whisper-small'
$modelRevision = '536b0662742c02347bc0e980a01041f333bce120'
$requirements = Join-Path $PSScriptRoot 'whisper-runtime-requirements.txt'

if ($Clean) {
    if (Test-Path -LiteralPath $destination) {
        $resolvedRepo = (Resolve-Path -LiteralPath $repoRoot).Path.TrimEnd('\')
        $resolvedDestination = (Resolve-Path -LiteralPath $destination).Path
        if (-not $resolvedDestination.StartsWith("$resolvedRepo\", [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove runtime outside repository: $resolvedDestination"
        }
        Remove-Item -LiteralPath $resolvedDestination -Recurse -Force
    }
    Write-Host 'Removed staged FUNG Whisper runtime.'
    exit 0
}

if ($PythonVersion -ne '3.11.9' -or $FasterWhisperVersion -ne '1.2.1' -or $Model -ne 'small') {
    throw 'This release script accepts only the reviewed Python 3.11.9 / faster-whisper 1.2.1 / small model set.'
}

if (-not $HostPython) {
    $uv = Get-Command uv -ErrorAction SilentlyContinue
    if ($uv) {
        $HostPython = (& $uv.Source python find 3.11).Trim()
    }
}
if (-not $HostPython -or -not (Test-Path -LiteralPath $HostPython -PathType Leaf)) {
    throw 'A Python 3.11 build interpreter is required. Pass -HostPython or install it with uv.'
}

New-Item -ItemType Directory -Path $cacheRoot -Force | Out-Null
$pythonArchive = Join-Path $cacheRoot "python-$PythonVersion-embed-amd64.zip"
if (-not (Test-Path -LiteralPath $pythonArchive -PathType Leaf)) {
    Invoke-WebRequest -Uri $pythonUrl -OutFile $pythonArchive
}
$actualPythonHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $pythonArchive).Hash.ToLowerInvariant()
if ($actualPythonHash -ne $pythonSha256) {
    throw "Python archive SHA-256 mismatch: expected $pythonSha256, got $actualPythonHash"
}

if (Test-Path -LiteralPath $destination) {
    Remove-Item -LiteralPath $destination -Recurse -Force
}
$scriptsDir = Join-Path $destination 'Scripts'
$sitePackages = Join-Path $destination 'Lib\site-packages'
$modelDir = Join-Path $destination 'models\small'
$licensesDir = Join-Path $destination 'LICENSES'
New-Item -ItemType Directory -Path $scriptsDir, $sitePackages, $modelDir, $licensesDir -Force | Out-Null
Expand-Archive -LiteralPath $pythonArchive -DestinationPath $scriptsDir -Force

$pthPath = Join-Path $scriptsDir 'python311._pth'
@(
    'python311.zip'
    '.'
    '..\Lib\site-packages'
    'import site'
) | Set-Content -LiteralPath $pthPath -Encoding ascii

& $HostPython -m pip install --disable-pip-version-check --no-deps --only-binary=:all: --require-hashes --target $sitePackages -r $requirements
if ($LASTEXITCODE -ne 0) {
    throw "Pinned runtime dependency install failed with exit code $LASTEXITCODE"
}

$embeddedPython = Join-Path $scriptsDir 'python.exe'
$downloadCode = @"
from huggingface_hub import snapshot_download
snapshot_download(repo_id='$modelRepo', revision='$modelRevision', local_dir=r'$modelDir')
"@
& $embeddedPython -c $downloadCode
if ($LASTEXITCODE -ne 0) {
    throw "Pinned Whisper model download failed with exit code $LASTEXITCODE"
}

$licenseSources = [ordered]@{
    'PYTHON-LICENSE.txt' = "https://raw.githubusercontent.com/python/cpython/v$PythonVersion/LICENSE"
    'FASTER-WHISPER-LICENSE.txt' = "https://raw.githubusercontent.com/SYSTRAN/faster-whisper/v$FasterWhisperVersion/LICENSE"
    'WHISPER-SMALL-MODEL-CARD.md' = "https://huggingface.co/$modelRepo/raw/$modelRevision/README.md"
}
foreach ($entry in $licenseSources.GetEnumerator()) {
    Invoke-WebRequest -Uri $entry.Value -OutFile (Join-Path $licensesDir $entry.Key)
}

$probeCode = @"
from faster_whisper import WhisperModel
model = WhisperModel(r'$modelDir', device='cpu', compute_type='int8')
print('portable-whisper-ready')
"@
$probeOutput = & $embeddedPython -c $probeCode
if ($LASTEXITCODE -ne 0 -or $probeOutput -notcontains 'portable-whisper-ready') {
    throw 'Portable Whisper runtime probe failed.'
}

$manifestPath = Join-Path $destination 'manifest.json'
$files = Get-ChildItem -LiteralPath $destination -Recurse -File | Where-Object {
    $_.FullName -ne $manifestPath
} | ForEach-Object {
    if (-not $_.FullName.StartsWith("$destination\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Runtime manifest encountered a file outside destination: $($_.FullName)"
    }
    [ordered]@{
        path = $_.FullName.Substring($destination.Length).TrimStart('\').Replace('\', '/')
        bytes = $_.Length
        sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
    }
}
$manifest = [ordered]@{
    schemaVersion = 1
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    profile = 'cpu-int8'
    python = [ordered]@{ version = $PythonVersion; source = $pythonUrl; sha256 = $pythonSha256 }
    fasterWhisper = [ordered]@{ version = $FasterWhisperVersion; requirements = 'scripts/whisper-runtime-requirements.txt' }
    model = [ordered]@{ name = $Model; repository = $modelRepo; revision = $modelRevision; license = 'MIT' }
    files = @($files)
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding utf8

Write-Host "Staged portable FUNG Whisper runtime at $destination"
Write-Host "Files: $($files.Count); bytes: $((Get-ChildItem -LiteralPath $destination -Recurse -File | Measure-Object Length -Sum).Sum)"
