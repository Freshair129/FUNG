param(
    [string]$PythonPath,
    [switch]$ReplaceExisting
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$sourceRoot = Join-Path $repoRoot ".venv-whisper\Scripts"
$runtimeRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot ".knowledge-parser-runtime"))
$expectedRuntimeRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot ".knowledge-parser-runtime"))
$pathComparison = [System.StringComparison]::OrdinalIgnoreCase
if (-not [string]::Equals($runtimeRoot, $expectedRuntimeRoot, $pathComparison) -or
    -not $runtimeRoot.StartsWith($repoRoot.TrimEnd('\') + '\', $pathComparison)) {
    throw "Parser runtime destination is outside the repository boundary."
}

if ([string]::IsNullOrWhiteSpace($PythonPath)) {
    $pythonCommand = Get-Command -Name python -CommandType Application -ErrorAction SilentlyContinue
    if ($null -eq $pythonCommand) {
        throw "A host Python interpreter with pip is required to stage the pinned parser dependency."
    }
    $PythonPath = $pythonCommand.Source
}
if (-not (Test-Path -LiteralPath $PythonPath -PathType Leaf)) {
    throw "The host Python path is unavailable."
}
if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot "python.exe") -PathType Leaf)) {
    throw "The bundled CPython 3.11.9 runtime is missing; PDF imports remain fail-closed."
}

if (Test-Path -LiteralPath $runtimeRoot) {
    $existing = @(Get-ChildItem -LiteralPath $runtimeRoot -Force -ErrorAction Stop)
    if ($existing.Count -gt 0 -and -not $ReplaceExisting) {
        throw "The parser runtime directory already contains files. Pass -ReplaceExisting only to rebuild this generated directory."
    }
    if ($ReplaceExisting) {
        $resolvedDestination = (Resolve-Path -LiteralPath $runtimeRoot).Path
        if (-not $resolvedDestination.StartsWith($repoRoot.TrimEnd('\') + '\', $pathComparison)) {
            throw "Refusing to replace a parser runtime outside the repository boundary."
        }
        Remove-Item -LiteralPath $resolvedDestination -Recurse -Force
    }
}

New-Item -ItemType Directory -Path $runtimeRoot -Force | Out-Null
Copy-Item -Path (Join-Path $sourceRoot "*") -Destination $runtimeRoot -Recurse -Force
$sitePackages = Join-Path $runtimeRoot "Lib\site-packages"
New-Item -ItemType Directory -Path $sitePackages -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "extract_knowledge.py") -Destination (Join-Path $runtimeRoot "extract_knowledge.py")

$pythonPathFile = Join-Path $runtimeRoot "python311._pth"
[System.IO.File]::WriteAllText(
    $pythonPathFile,
    "python311.zip`r`n.`r`nLib\site-packages`r`nimport site`r`n",
    [System.Text.Encoding]::ASCII
)

$embeddedPython = Join-Path $runtimeRoot "python.exe"
$embeddedVersion = (& $embeddedPython -I -B -c "import sys; print('.'.join(map(str, sys.version_info[:3])))").Trim()
if ($LASTEXITCODE -ne 0 -or $embeddedVersion -ne "3.11.9") {
    throw "Parser runtime CPython version must be 3.11.9; observed '$embeddedVersion'."
}

$requirements = Join-Path $PSScriptRoot "knowledge-extraction-requirements.txt"
& $PythonPath -m pip install --disable-pip-version-check --no-deps --target $sitePackages -r $requirements
if ($LASTEXITCODE -ne 0) {
    throw "Could not stage the pinned pypdf parser dependency."
}

$icacls = Get-Command -Name icacls.exe -CommandType Application -ErrorAction SilentlyContinue
if ($null -eq $icacls) {
    throw "Could not restore inherited access to the staged parser dependency."
}
& $icacls.Source $sitePackages /inheritance:e /T /C /Q
if ($LASTEXITCODE -ne 0) {
    throw "Could not restore inherited access to the staged parser dependency."
}

$metadata = & $embeddedPython -I -B -c "import importlib.metadata, pypdf; d=importlib.metadata.distribution('pypdf'); print(d.version + '|' + (d.metadata.get('License-Expression') or d.metadata.get('License') or ''))"
if ($LASTEXITCODE -ne 0 -or $metadata.Trim() -notmatch '^6\.10\.0\|') {
    throw "The staged parser dependency does not match the pinned pypdf version."
}
$licenseText = ($metadata.Trim() -split '\|', 2)[1]
if ($licenseText -notin @("BSD-3-Clause", "BSD 3-Clause License")) {
    throw "The staged pypdf license does not match the accepted BSD-3-Clause license."
}
$manifest = @{
    contractVersion = 1
    pythonVersion = "3.11.9"
    pypdfVersion = "6.10.0"
    pypdfLicense = "BSD-3-Clause"
}
$manifestJson = ConvertTo-Json -InputObject $manifest -Compress
[System.IO.File]::WriteAllText(
    (Join-Path $runtimeRoot "parser-runtime.manifest.json"),
    $manifestJson,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Staged isolated local knowledge parser runtime (CPython 3.11.9, pypdf 6.10.0, BSD-3-Clause)."
