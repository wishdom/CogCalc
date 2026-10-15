$ErrorActionPreference = 'Stop'

# Run from any directory; honor Cargo's configured target directory.
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
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
    New-Item -ItemType Directory -Path $packageDirectory -Force | Out-Null
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $packageDirectory 'CogCalc.exe') -Force
    Copy-Item -LiteralPath 'LICENSE', 'README.md' -Destination $packageDirectory -Force
    $imageDirectory = Join-Path $packageDirectory 'docs'
    foreach ($previewPath in @('docs/preview.png', 'docs/preview-dark.png', 'docs/preview-drivetrain.png')) {
        if (Test-Path -LiteralPath $previewPath) {
            New-Item -ItemType Directory -Path $imageDirectory -Force | Out-Null
            Copy-Item -LiteralPath $previewPath -Destination $imageDirectory -Force
        }
    }
    $archivePath = Join-Path $projectRoot 'dist/CogCalc-windows.zip'
    Compress-Archive -LiteralPath $packageDirectory -DestinationPath $archivePath -Force
    Write-Output "Portable build: $packageDirectory"
    Write-Output "Archive: $archivePath"
} finally {
    Pop-Location
}
