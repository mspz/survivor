# Build a portable Windows player ZIP, including the local WoW MPQ data.
# Requires Windows PowerShell 5.1+ (or PowerShell 7), Rust and MSVC build tools.
[CmdletBinding()]
param(
    [string]$DataPath,
    [string]$OutputPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$targetTriple = 'x86_64-pc-windows-msvc'
$targetDir = Join-Path $repoRoot 'target'

if ($env:OS -ne 'Windows_NT') {
    throw 'Run this script on Windows with the MSVC Rust toolchain.'
}
if (-not $DataPath) {
    if ($env:WOW_DATA) { $DataPath = $env:WOW_DATA }
    else { $DataPath = Join-Path $repoRoot 'wow-data/Data' }
}
$dataRoot = (Resolve-Path -LiteralPath $DataPath).ProviderPath
foreach ($required in @('terrain.MPQ', 'patch.MPQ')) {
    if (-not (Test-Path -LiteralPath (Join-Path $dataRoot $required) -PathType Leaf)) {
        throw "Missing $required in $dataRoot. Run bash scripts/fetch-data.sh first, or pass -DataPath pointing to a WoW 1.12.1 Data folder."
    }
}
$dataFiles = @(Get-ChildItem -LiteralPath $dataRoot -Recurse -File | Where-Object { $_.Extension -ieq '.mpq' })
if (-not $OutputPath) {
    $OutputPath = Join-Path $repoRoot ("dist/WarcraftSurvivors-windows-{0}.zip" -f (Get-Date -Format 'yyyyMMdd-HHmmss'))
}
$zipPath = [System.IO.Path]::GetFullPath($OutputPath)
if (Test-Path -LiteralPath $zipPath) { throw "Output already exists: $zipPath. Choose another -OutputPath." }

Push-Location $repoRoot
try {
    Write-Host 'Building the optimized Windows player (without development tools)...'
    & cargo build --locked --profile play --package warcraft-survivors --no-default-features --target $targetTriple --target-dir $targetDir
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed (exit code $LASTEXITCODE)." }
} finally {
    Pop-Location
}
$exePath = Join-Path $targetDir "$targetTriple/play/warcraft-survivors.exe"
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) { throw "Build did not produce $exePath." }

# ZipArchive streams entries and supports ZIP64; Compress-Archive has a 2 GB/file limit.
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
function Add-ZipFile($Archive, [string]$Source, [string]$Name, $Compression) {
    $entry = $Archive.CreateEntry($Name, $Compression)
    $sourceStream = [System.IO.File]::OpenRead($Source)
    try {
        $destination = $entry.Open()
        try { $sourceStream.CopyTo($destination) } finally { $destination.Dispose() }
    } finally { $sourceStream.Dispose() }
}
function Add-ZipText($Archive, [string]$Name, [string]$Content) {
    $entry = $Archive.CreateEntry($Name)
    $writer = [System.IO.StreamWriter]::new($entry.Open(), [System.Text.UTF8Encoding]::new($false))
    try { $writer.Write($Content) } finally { $writer.Dispose() }
}

$null = New-Item -ItemType Directory -Path (Split-Path -Parent $zipPath) -Force
$stream = [System.IO.File]::Open($zipPath, [System.IO.FileMode]::CreateNew)
$complete = $false
try {
    $archive = [System.IO.Compression.ZipArchive]::new($stream, [System.IO.Compression.ZipArchiveMode]::Create, $true)
    try {
        $prefix = 'WarcraftSurvivors/'
        Add-ZipFile $archive $exePath ($prefix + 'warcraft-survivors.exe') ([System.IO.Compression.CompressionLevel]::Optimal)
        foreach ($file in Get-ChildItem -LiteralPath (Join-Path $repoRoot 'config') -Filter 'survivors-*.toml' -File) {
            Add-ZipFile $archive $file.FullName ($prefix + 'config/' + $file.Name) ([System.IO.Compression.CompressionLevel]::Optimal)
        }
        foreach ($license in @('LICENSE-MIT', 'LICENSE-APACHE')) {
            Add-ZipFile $archive (Join-Path $repoRoot $license) ($prefix + $license) ([System.IO.Compression.CompressionLevel]::Optimal)
        }
        Add-ZipText $archive ($prefix + 'Jouer.bat') (@'
@echo off
setlocal
cd /d "%~dp0"
set "WOW_DATA=%~dp0Data"
set "BENILLA_HOME=%~dp0benilla-config"
"%~dp0warcraft-survivors.exe" %*
if errorlevel 1 pause
'@ -replace "`r?`n", "`r`n")
        Add-ZipText $archive ($prefix + 'LIRE-MOI.txt') @'
Extraire entierement cette archive, puis lancer Jouer.bat.
Windows 64 bits avec un pilote graphique recent est necessaire.
Rust et les outils de compilation ne sont pas necessaires pour jouer.
Les donnees WoW sont incluses dans Data/. Ne pas lancer depuis le ZIP.
Les sauvegardes et reglages sont crees dans benilla-config/.
Les fichiers config/survivors-*.toml permettent de regler le jeu.
Les licences incluses concernent le code du projet, pas les donnees WoW.
'@
        $index = 0
        foreach ($file in $dataFiles) {
            $index++
            Write-Progress -Activity 'Adding WoW data to ZIP' -Status $file.Name -PercentComplete (100 * $index / $dataFiles.Count)
            $relative = $file.FullName.Substring($dataRoot.TrimEnd('\', '/').Length + 1).Replace('\', '/')
            # MPQs are already compressed; avoid spending time recompressing several GB.
            Add-ZipFile $archive $file.FullName ($prefix + 'Data/' + $relative) ([System.IO.Compression.CompressionLevel]::NoCompression)
        }
    } finally { $archive.Dispose() }
    $complete = $true
} finally {
    $stream.Dispose()
    Write-Progress -Activity 'Adding WoW data to ZIP' -Completed
    if (-not $complete) { Remove-Item -LiteralPath $zipPath -Force }
}
Write-Host "Archive ready: $zipPath"
Write-Host ('Size: {0:N2} GB' -f ((Get-Item -LiteralPath $zipPath).Length / 1GB))
