<#
.SYNOPSIS
    Installs the speaker-diarization dependencies into the staged FUNG Whisper
    runtime, and optionally warms the gated model cache.

.DESCRIPTION
    Diarization is deliberately NOT part of the default bundle, for two
    reasons that are worth stating rather than discovering:

      1. Size. `pyannote.audio` pulls `torch`, `torchaudio`, `lightning`, and
         their transitive tree. That is hundreds of megabytes on CPU and
         several gigabytes with CUDA wheels — multiples of the entire rest of
         the FUNG installer, imposed on every user for a feature most will
         not use.

      2. Licence. `pyannote/speaker-diarization-3.1` is gated on Hugging
         Face. Each user must accept its terms under their own account, and
         weights obtained that way may not be redistributed inside an
         installer. This is a licence constraint, not an oversight, and it is
         why the Whisper model ships pinned in `stage_whisper_runtime.ps1`
         while this one does not.

    So this script is opt-in, run once, on top of an already-staged runtime.

.PARAMETER GenerateLock
    Resolve the dependency tree with pip and write a hash-pinned lockfile to
    `scripts/diarization-runtime-requirements.txt`, then stop without
    installing.

    This step exists because the pinned set CANNOT be hand-written. Unlike the
    Whisper requirements, whose two dozen entries were resolved once and
    reviewed, the pyannote tree is large, platform-dependent, and changes with
    the torch build selected. Distribution digests can only be obtained by
    resolving and downloading the tree — so the lockfile is generated on a
    machine with network access, reviewed, and committed, exactly like any
    other dependency change. The legacy pure-Python dependencies are built
    into wheels first because pip's cross-target resolver accepts only wheels.
    The final lock records their original source hashes so a clean staging
    machine does not depend on a non-reproducible build-wheel timestamp.

.PARAMETER FetchModel
    After installing, download the gated pipeline into FUNG's own Hugging Face
    cache so later runs are offline. Requires FUNG_HF_TOKEN or HF_TOKEN, and
    requires that you have already accepted the licences for BOTH:
        https://huggingface.co/pyannote/speaker-diarization-3.1
        https://huggingface.co/pyannote/segmentation-3.0
    (the 3.1 pipeline loads segmentation-3.0 as a component).

.PARAMETER CacheRoot
    Where to place the model cache. Defaults to the same location the running
    app uses, so a model fetched here is the one the app finds. Override only
    if you also set FUNG_HF_HOME for the app.

.PARAMETER HostPython
    Build/install interpreter with pip. The shipped Python is an embedded
    runtime and intentionally has no pip, so this must be a separate Python
    installation (or uv-managed interpreter). Wheels are resolved for the
    bundled CPython 3.11 runtime even when HostPython itself is another
    supported version.

.PARAMETER TorchVersion
    Torch version pinned for pyannote.audio compatibility. Keep paired with
    TorchaudioVersion unless the import probe and model smoke are rerun.

.PARAMETER TorchaudioVersion
    Torchaudio version paired with TorchVersion. pyannote.audio 3.4.0 uses
    the AudioMetaData API retained by the reviewed 2.4.1 pair.

.EXAMPLE
    # One-time, on a machine with network access:
    ./scripts/stage_diarization_runtime.ps1 -GenerateLock
    # review + commit scripts/diarization-runtime-requirements.txt, then:
    ./scripts/stage_diarization_runtime.ps1 -FetchModel
#>
[CmdletBinding()]
param(
    [string]$PyannoteVersion = '3.4.0',
    [string]$TorchVersion = '2.4.1',
    [string]$TorchaudioVersion = '2.4.1',
    [ValidateSet('cpu', 'cu121')]
    [string]$TorchVariant = 'cpu',
    [string]$HostPython,
    [string]$CacheRoot,
    [switch]$GenerateLock,
    [switch]$FetchModel
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$venv = Join-Path $repoRoot '.venv-whisper'
$python = Join-Path $venv 'Scripts\python.exe'
$sitePackages = Join-Path $venv 'Lib\site-packages'
$lockfile = Join-Path $PSScriptRoot 'diarization-runtime-requirements.txt'
$licensesDir = Join-Path $venv 'LICENSES'
$model = 'pyannote/speaker-diarization-3.1'

if (-not (Test-Path -LiteralPath $python -PathType Leaf)) {
    throw "No staged FUNG runtime at $venv. Run scripts/stage_whisper_runtime.ps1 first."
}

if (-not $HostPython) {
    $uv = Get-Command uv -ErrorAction SilentlyContinue
    if ($uv) {
        $HostPython = (& $uv.Source python find 3.11).Trim()
    }
}
if (-not $HostPython) {
    $pythonCommand = Get-Command python -ErrorAction SilentlyContinue
    if ($pythonCommand) {
        $HostPython = $pythonCommand.Source
    }
}
if (-not $HostPython -or -not (Test-Path -LiteralPath $HostPython -PathType Leaf)) {
    throw 'A host Python with pip is required. Pass -HostPython or install Python/uv.'
}

# The shipped interpreter is the embeddable CPython 3.11 runtime and has no
# pip. Resolve and install wheels with the host interpreter, but target the
# ABI/platform that the app actually runs. The cross-target resolver accepts
# only wheels; source-only pure-Python dependencies are supplied as temporary
# resolver wheels and remain source-hash pinned in the final lock.
$targetArgs = @(
    '--platform', 'win_amd64',
    '--python-version', '3.11',
    '--implementation', 'cp',
    '--abi', 'cp311'
)

# The CPU wheels come from PyPI; the CUDA ones only from PyTorch's own index.
# Naming the index explicitly keeps the choice visible in the generated
# lockfile rather than depending on whatever the operator's pip.conf says.
$indexArgs = @()
if ($TorchVariant -ne 'cpu') {
    $indexArgs = @('--extra-index-url', "https://download.pytorch.org/whl/$TorchVariant")
}

if ($GenerateLock) {
    $work = Join-Path $repoRoot ".runtime-cache\diarization-lock"
    if (Test-Path -LiteralPath $work) {
        Get-ChildItem -LiteralPath $work -Force | Remove-Item -Recurse -Force
    }
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    Write-Host "Resolving pyannote.audio==$PyannoteVersion ($TorchVariant) ..."

    # These dependencies are pure Python but PyPI publishes the versions
    # selected by this tree as sdists. Build them into universal wheels so the
    # resolver can still target the embedded CPython 3.11 runtime without
    # accepting arbitrary native source builds into the app bundle.
    $sourceDependencies = @(
        'antlr4-python3-runtime==4.9.3'
        'docopt==0.6.2'
    )
    $sourcePackageNames = $sourceDependencies | ForEach-Object {
        ($_ -split '==', 2)[0].ToLowerInvariant().Replace('_', '-')
    }
    $constraints = Join-Path $work 'constraints.txt'
    @(
        "torch==$TorchVersion"
        "torchaudio==$TorchaudioVersion"
    ) | Set-Content -LiteralPath $constraints -Encoding ascii
    foreach ($sourceDependency in $sourceDependencies) {
        & $HostPython -m pip wheel --disable-pip-version-check --no-deps `
            --wheel-dir $work $sourceDependency
        if ($LASTEXITCODE -ne 0) {
            throw "Building pure-Python resolver wheel $sourceDependency failed with exit code $LASTEXITCODE"
        }
    }

    # `pip download` resolves the full transitive tree and gives us real
    # wheels to hash. Nothing is installed yet: the point is to produce a
    # reviewable artifact, not to mutate the runtime.
    & $HostPython -m pip download --disable-pip-version-check --only-binary=:all: `
        --dest $work --find-links $work --constraint $constraints @targetArgs @indexArgs "pyannote.audio==$PyannoteVersion"
    if ($LASTEXITCODE -ne 0) { throw "Dependency resolution failed with exit code $LASTEXITCODE" }

    # Save source distributions separately for the two packages above. Their
    # source hashes, rather than the timestamp-sensitive helper wheel hashes,
    # are what the install lock records.
    $sourceWork = Join-Path $work 'source'
    New-Item -ItemType Directory -Path $sourceWork -Force | Out-Null
    & $HostPython -m pip download --disable-pip-version-check --no-deps --no-binary=:all: `
        --dest $sourceWork @sourceDependencies
    if ($LASTEXITCODE -ne 0) { throw "Downloading source hashes failed with exit code $LASTEXITCODE" }

    $lines = @(
        "# Generated by scripts/stage_diarization_runtime.ps1 -GenerateLock"
        "# torch variant: $TorchVariant   pyannote.audio: $PyannoteVersion   torch: $TorchVersion   torchaudio: $TorchaudioVersion"
        "# Review this file like any other dependency change before committing."
    )
    Get-ChildItem -LiteralPath $work -Filter '*.whl' -File | Sort-Object Name | ForEach-Object {
        # Wheel filenames are `{name}-{version}-{tags}.whl`. The distribution
        # name may contain hyphens, so split at the first version-looking
        # field rather than treating the first hyphen as the separator.
        $match = [regex]::Match($_.Name, '^(?<name>.+?)-(?<version>\d[0-9A-Za-z.+!]*)(?:-[^-]+){3}\.whl$')
        if (-not $match.Success) { throw "Unexpected wheel name: $($_.Name)" }
        $name = $match.Groups['name'].Value
        $version = $match.Groups['version'].Value
        if ($sourcePackageNames -contains $name.ToLowerInvariant().Replace('_', '-')) {
            return
        }
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
        $lines += "$name==$version --hash=sha256:$hash"
    }
    foreach ($sourceDependency in $sourceDependencies) {
        $parts = $sourceDependency -split '==', 2
        $package = $parts[0]
        $version = $parts[1]
        $sourceFile = Get-ChildItem -LiteralPath $sourceWork -File |
            Where-Object { $_.Name -match ("^" + [regex]::Escape($package) + "-" + [regex]::Escape($version) + "\.(tar\.gz|zip)$") } |
            Select-Object -First 1
        if (-not $sourceFile) { throw "Could not find source archive for $sourceDependency" }
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sourceFile.FullName).Hash.ToLowerInvariant()
        $lines += "$package==$version --hash=sha256:$hash"
    }
    $lines | Set-Content -LiteralPath $lockfile -Encoding utf8
    Write-Host "Wrote $lockfile ($($lines.Count - 3) pinned distributions)."
    Write-Host 'Review and commit it, then re-run this script without -GenerateLock.'
    exit 0
}

if (-not (Test-Path -LiteralPath $lockfile -PathType Leaf)) {
    throw @"
No pinned lockfile at $lockfile.

The diarization dependency tree cannot be hand-written: its wheel digests are
only obtainable by downloading the wheels. Run this script with -GenerateLock
on a machine with network access, review the result, and commit it.
"@
}

# `--target` overlays a shared site-packages directory. When a previous
# resolution used a different Torch/Hugging Face version, pip overwrites the
# importable package but leaves the old `.dist-info` metadata behind. That
# makes `importlib.metadata` report multiple versions and can make a later
# worker choose the wrong dependency metadata. Remove only stale metadata for
# distributions owned by this reviewed lock; unrelated Whisper packages stay
# untouched.
function Normalize-PackageName([string]$name) {
    return ($name.ToLowerInvariant() -replace '[-_.]+', '-')
}

$lockedVersions = @{}
foreach ($line in (Get-Content -LiteralPath $lockfile)) {
    if ($line -match '^([A-Za-z0-9_.-]+)==([^\s]+)\s+--hash=sha256:') {
        $lockedVersions[(Normalize-PackageName $Matches[1])] = $Matches[2]
    }
}
foreach ($metadata in (Get-ChildItem -LiteralPath $sitePackages -Directory -Filter '*.dist-info' -ErrorAction SilentlyContinue)) {
    if ($metadata.Name -notmatch '^(?<name>.+)-(?<version>[^-]+)\.dist-info$') { continue }
    $normalized = Normalize-PackageName $Matches['name']
    $expected = $lockedVersions[$normalized]
    if ($expected -and $Matches['version'] -ne $expected) {
        Remove-Item -LiteralPath $metadata.FullName -Recurse -Force
        Write-Host "Removed stale metadata $($metadata.Name)"
    }
}

Write-Host "Installing diarization dependencies from $lockfile ..."
& $HostPython -m pip install --disable-pip-version-check --upgrade --no-deps `
    --require-hashes --target $sitePackages @targetArgs -r $lockfile
if ($LASTEXITCODE -ne 0) { throw "Diarization dependency install failed with exit code $LASTEXITCODE" }

New-Item -ItemType Directory -Path $licensesDir -Force | Out-Null
$licenseSources = [ordered]@{
    'PYANNOTE-AUDIO-LICENSE.txt' = 'https://raw.githubusercontent.com/pyannote/pyannote-audio/develop/LICENSE'
    'PYTORCH-LICENSE.txt'        = 'https://raw.githubusercontent.com/pytorch/pytorch/main/LICENSE'
}
foreach ($entry in $licenseSources.GetEnumerator()) {
    Invoke-WebRequest -Uri $entry.Value -OutFile (Join-Path $licensesDir $entry.Key)
}

# Import rather than assume: the whole point of the readiness probe is that
# "the directory exists" and "the package loads" are different claims, and
# staging is the one place that can afford to check the stronger one.
$probe = & $python -c "import torch, pyannote.audio; print('diarization-deps-ready')"
if ($LASTEXITCODE -ne 0 -or $probe -notcontains 'diarization-deps-ready') {
    throw 'Diarization dependencies installed but do not import.'
}
Write-Host 'Diarization dependencies staged.'

if (-not $FetchModel) {
    Write-Host ''
    Write-Host 'Model not fetched. Re-run with -FetchModel once you have:'
    Write-Host "  1. accepted https://huggingface.co/$model"
    Write-Host '  2. accepted https://huggingface.co/pyannote/segmentation-3.0'
    Write-Host '  3. set FUNG_HF_TOKEN (or HF_TOKEN)'
    exit 0
}

$token = $env:FUNG_HF_TOKEN
if (-not $token) { $token = $env:HF_TOKEN }
if (-not $token) {
    throw 'FUNG_HF_TOKEN (or HF_TOKEN) is required to fetch the gated model.'
}

if (-not $CacheRoot) {
    # Mirrors `diarization::hf_home`: the app looks here, so staging must too,
    # or the download lands somewhere the probe will never find.
    if ($env:FUNG_HF_HOME) {
        $CacheRoot = $env:FUNG_HF_HOME
    } else {
        $CacheRoot = Join-Path $env:APPDATA 'com.fung.desktop\huggingface'
    }
}
New-Item -ItemType Directory -Path $CacheRoot -Force | Out-Null
Write-Host "Fetching $model into $CacheRoot ..."

$env:HF_HOME = $CacheRoot
$fetch = @"
from pyannote.audio import Pipeline
Pipeline.from_pretrained('$model', use_auth_token='$token')
print('diarization-model-ready')
"@
$fetchOutput = & $python -c $fetch
if ($LASTEXITCODE -ne 0 -or $fetchOutput -notcontains 'diarization-model-ready') {
    throw "Model fetch failed. Confirm both licences are accepted for this account."
}
Write-Host "Staged diarization model at $CacheRoot"
