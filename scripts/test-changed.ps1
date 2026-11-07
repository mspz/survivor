# Windows entry point; skip Windows Store aliases that cannot launch Python here.
$ErrorActionPreference = 'Stop'
$candidates = @()
foreach ($name in @('python3', 'python')) {
    $command = Get-Command $name -ErrorAction SilentlyContinue
    if ($command -and $command.Source -notlike '*\WindowsApps\*') {
        $candidates += $command.Source
    }
}
$pythonRoot = Join-Path $env:LOCALAPPDATA 'Programs\Python'
if (Test-Path -LiteralPath $pythonRoot) {
    $candidates += Get-ChildItem -LiteralPath $pythonRoot -Directory |
        ForEach-Object { Join-Path $_.FullName 'python.exe' } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }
}
$python = $candidates | Select-Object -First 1
if (-not $python) { throw 'Python 3.10+ is required; run scripts/test-changed.py with your Python executable.' }
& $python (Join-Path $PSScriptRoot 'test-changed.py') @args
exit $LASTEXITCODE
