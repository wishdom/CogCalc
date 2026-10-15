param(
    [switch]$SkipBuild,
    [string]$LegalDirectory
)

$ErrorActionPreference = 'Stop'

# Run from any directory; honor Cargo's configured target directory.
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    if (-not $SkipBuild) {
        cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    }
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
    if ($LegalDirectory) {
        Copy-Item -LiteralPath (Join-Path $LegalDirectory 'THIRD-PARTY-NOTICES.txt'), (Join-Path $LegalDirectory 'SOURCE.txt') -Destination $packageDirectory -Force
    }
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
    Pop-Location
}
