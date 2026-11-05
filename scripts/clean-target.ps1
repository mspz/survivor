# Only generated incremental caches, never binaries, dependencies, probes or game data.
[CmdletBinding(SupportsShouldProcess)]
param([string[]]$Profiles = @('debug', 'play'))
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$target = [IO.Path]::GetFullPath((Join-Path $workspace 'target'))
foreach ($profile in $Profiles) {
    if ($profile -notin @('debug', 'play', 'release', 'ship')) { throw "Unsupported profile: $profile" }
}
# Deleting a live compiler cache would invalidate an in-progress build.
if (Get-Process cargo,rustc -ErrorAction SilentlyContinue) {
    throw 'A Cargo/rustc process is running. Finish the build before cleaning its cache.'
}
if (-not (Test-Path -LiteralPath $target)) { return }
# Refuse junctions and symbolic links, including in ancestors of an incremental cache.
if ((Get-Item -LiteralPath $target).Attributes -band [IO.FileAttributes]::ReparsePoint) {
    throw 'Refusing a linked target directory.'
}
$freed = 0L
foreach ($profile in ($Profiles | Select-Object -Unique)) {
    $profilePath = [IO.Path]::GetFullPath((Join-Path $target $profile))
    $cache = [IO.Path]::GetFullPath((Join-Path $profilePath 'incremental'))
    if (-not $cache.StartsWith($target + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Cache escapes workspace target: $cache"
    }
    if (-not (Test-Path -LiteralPath $cache)) { continue }
    $directories = @((Get-Item -LiteralPath $profilePath), (Get-Item -LiteralPath $cache))
    $directories += Get-ChildItem -LiteralPath $cache -Directory -Recurse -Force
    if ($directories | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }) {
        throw "Refusing linked directories in $cache"
    }
    $bytes = (Get-ChildItem -LiteralPath $cache -File -Recurse -Force | Measure-Object Length -Sum).Sum
    Write-Output ('{0}: {1:N2} GiB' -f $cache, ($bytes / 1GB))
    if ($PSCmdlet.ShouldProcess($cache, 'Delete generated incremental compiler cache')) {
        Remove-Item -LiteralPath $cache -Recurse -Force
        $freed += $bytes
    }
}
Write-Output ('Freed {0:N2} GiB. The next build recreates any cache it needs.' -f ($freed / 1GB))
