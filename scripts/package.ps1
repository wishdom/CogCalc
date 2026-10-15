[CmdletBinding()]
param(
    [string]$LegalDirectory = 'dist/legal'
)

$ErrorActionPreference = 'Stop'

# Run from any directory; honor Cargo's configured target directory.
$projectRoot = Split-Path -Parent $PSScriptRoot
$previousLegalDirectory = [Environment]::GetEnvironmentVariable('COGCALC_LEGAL_DIR', 'Process')
Push-Location -LiteralPath $projectRoot
try {
    $legalHelp = 'Generate matching legal files with python scripts/package-source.py --tag v<VERSION> before packaging.'
    if (-not (Test-Path -LiteralPath $LegalDirectory -PathType Container)) {
        throw "Legal directory '$LegalDirectory' was not found. $legalHelp"
    }
    $resolvedLegalDirectory = (Resolve-Path -LiteralPath $LegalDirectory).ProviderPath
    foreach ($legalName in @('THIRD-PARTY-NOTICES.txt', 'SOURCE.txt')) {
        $legalPath = Join-Path $resolvedLegalDirectory $legalName
        if (-not (Test-Path -LiteralPath $legalPath -PathType Leaf) -or (Get-Item -LiteralPath $legalPath).Length -eq 0) {
            throw "Legal directory must contain nonempty $legalName. $legalHelp"
        }
    }
    $env:COGCALC_LEGAL_DIR = $resolvedLegalDirectory
    # Always let Cargo rebuild when embedded notices or source information change.
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    $metadataText = cargo metadata --no-deps --format-version 1 --locked
    if ($LASTEXITCODE -ne 0) { throw 'Unable to read Cargo metadata.' }
    $metadata = $metadataText | ConvertFrom-Json
    $binaryPath = Join-Path $metadata.target_directory 'release/cogcalc.exe'
    if (-not (Test-Path -LiteralPath $binaryPath)) {
        throw 'This packaging script requires a Windows Rust host.'
    }
    $packageDirectory = Join-Path $projectRoot 'dist/CogCalc'
    # Only remove the fixed staging directory inside this repository.
    $resolvedPackageDirectory = [IO.Path]::GetFullPath($packageDirectory)
    $expectedPackageDirectory = [IO.Path]::GetFullPath((Join-Path $projectRoot 'dist/CogCalc'))
    if ($resolvedPackageDirectory -ne $expectedPackageDirectory) { throw 'Unexpected package directory.' }
    if (Test-Path -LiteralPath $resolvedPackageDirectory) {
        Remove-Item -LiteralPath $resolvedPackageDirectory -Recurse -Force
    }
    New-Item -ItemType Directory -Path $packageDirectory -Force | Out-Null
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $packageDirectory 'CogCalc.exe') -Force
    Copy-Item -LiteralPath 'LICENSE', 'README.md' -Destination $packageDirectory -Force
    Copy-Item -LiteralPath (Join-Path $resolvedLegalDirectory 'THIRD-PARTY-NOTICES.txt'), (Join-Path $resolvedLegalDirectory 'SOURCE.txt') -Destination $packageDirectory -Force
    $imageDirectory = Join-Path $packageDirectory 'docs'
    foreach ($previewPath in @('docs/preview.png', 'docs/preview-dark.png', 'docs/preview-drivetrain.png')) {
        if (Test-Path -LiteralPath $previewPath) {
            New-Item -ItemType Directory -Path $imageDirectory -Force | Out-Null
            Copy-Item -LiteralPath $previewPath -Destination $imageDirectory -Force
        }
    }
    $archivePath = Join-Path $projectRoot 'dist/CogCalc-windows-x64.zip'
    Compress-Archive -LiteralPath $packageDirectory -DestinationPath $archivePath -Force
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $projectRoot 'dist/CogCalc.exe') -Force
    Write-Output "Portable build: $packageDirectory"
    Write-Output "Archive: $archivePath"
} finally {
    [Environment]::SetEnvironmentVariable('COGCALC_LEGAL_DIR', $previousLegalDirectory, 'Process')
    Pop-Location
}
