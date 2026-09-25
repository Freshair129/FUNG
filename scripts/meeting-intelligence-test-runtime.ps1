[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ManifestPath,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$TargetDir,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$PythonPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$environmentNames = @(
    'CARGO_TARGET_DIR',
    'CARGO_PROFILE_DEV_DEBUG',
    'CARGO_PROFILE_TEST_DEBUG',
    'CARGO_INCREMENTAL',
    'TAURI_CONFIG',
    'PATH'
)
$originalEnvironment = @{}
$cargoExitCode = 1

function Get-ProcessEnvironmentSnapshot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $environmentItem = Get-Item -LiteralPath ("Env:{0}" -f $Name) -ErrorAction SilentlyContinue
    if ($null -eq $environmentItem) {
        return [pscustomobject]@{
            Exists = $false
            Value  = $null
        }
    }

    return [pscustomobject]@{
        Exists = $true
        Value  = [string]$environmentItem.Value
    }
}

function Restore-ProcessEnvironmentValue {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [psobject]$Snapshot
    )

    $environmentPath = "Env:{0}" -f $Name
    if ($Snapshot.Exists) {
        Set-Item -LiteralPath $environmentPath -Value $Snapshot.Value
    } else {
        Remove-Item -LiteralPath $environmentPath -ErrorAction SilentlyContinue
    }
}

try {
    $manifestItem = Get-Item -LiteralPath $ManifestPath -ErrorAction SilentlyContinue
    if ($null -eq $manifestItem -or $manifestItem.PSIsContainer) {
        throw "ManifestPath must identify an existing file: $ManifestPath"
    }

    $targetItem = Get-Item -LiteralPath $TargetDir -ErrorAction SilentlyContinue
    if ($null -ne $targetItem -and -not $targetItem.PSIsContainer) {
        throw "TargetDir exists but is not a directory: $TargetDir"
    }
    if ($null -eq $targetItem) {
        [System.IO.Directory]::CreateDirectory($TargetDir) | Out-Null
        $targetItem = Get-Item -LiteralPath $TargetDir -ErrorAction Stop
    }

    $pythonItem = Get-Item -LiteralPath $PythonPath -ErrorAction SilentlyContinue
    if ($null -eq $pythonItem -or $pythonItem.PSIsContainer) {
        throw "PythonPath must identify an existing file: $PythonPath"
    }

    $cargoCommand = Get-Command -Name 'cargo' -CommandType Application -ErrorAction SilentlyContinue
    if ($null -eq $cargoCommand) {
        throw 'cargo is not available as an executable on the current process PATH'
    }

    $cargoPath = [string]$cargoCommand.Path
    if ([string]::IsNullOrWhiteSpace($cargoPath)) {
        $cargoPath = [string]$cargoCommand.Source
    }
    if ([string]::IsNullOrWhiteSpace($cargoPath)) {
        throw 'cargo was found but its executable path could not be resolved'
    }

    $manifestFullPath = $manifestItem.FullName
    $targetFullPath = $targetItem.FullName
    $pythonFullPath = $pythonItem.FullName
    $pythonParentPath = [IO.Path]::GetDirectoryName($pythonFullPath)
    if ([string]::IsNullOrWhiteSpace($pythonParentPath)) {
        throw "PythonPath has no usable parent directory: $pythonFullPath"
    }

    foreach ($environmentName in $environmentNames) {
        $originalEnvironment[$environmentName] = Get-ProcessEnvironmentSnapshot -Name $environmentName
    }

    try {
        $env:CARGO_TARGET_DIR = $targetFullPath
        $env:CARGO_PROFILE_DEV_DEBUG = '0'
        $env:CARGO_PROFILE_TEST_DEBUG = '0'
        $env:CARGO_INCREMENTAL = '0'
        $env:TAURI_CONFIG = '{"bundle":{"resources":[]}}'

        $existingPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
        if ([string]::IsNullOrEmpty($existingPath)) {
            $env:PATH = $pythonParentPath
        } else {
            $env:PATH = $pythonParentPath + [IO.Path]::PathSeparator + $existingPath
        }

        Write-Host ("Running Cargo library regression with manifest {0}" -f $manifestFullPath)
        Write-Host ("Using Cargo target directory {0}" -f $targetFullPath)
        Write-Host ("Using process-local Python directory {0}" -f $pythonParentPath)

        $cargoArguments = @(
            'test',
            '--quiet',
            '--manifest-path',
            $manifestFullPath,
            '--offline',
            '--locked',
            '--lib',
            '--',
            '--color',
            'never'
        )
        & $cargoPath @cargoArguments
        $cargoExitCode = [int]$LASTEXITCODE
        if ($cargoExitCode -eq 0) {
            Write-Host 'Running the meeting-knowledge behavioral integration suite'
            $knowledgeArguments = @(
                'test',
                '--quiet',
                '--manifest-path',
                $manifestFullPath,
                '--offline',
                '--locked',
                '--test',
                'meeting_knowledge',
                '--',
                '--color',
                'never'
            )
            & $cargoPath @knowledgeArguments
            $cargoExitCode = [int]$LASTEXITCODE
        }
    } finally {
        foreach ($environmentName in $environmentNames) {
            Restore-ProcessEnvironmentValue -Name $environmentName -Snapshot $originalEnvironment[$environmentName]
        }
    }
} catch {
    Write-Error ("meeting-intelligence-test-runtime.ps1 failed closed: {0}" -f $_.Exception.Message)
    exit 1
}

exit $cargoExitCode
