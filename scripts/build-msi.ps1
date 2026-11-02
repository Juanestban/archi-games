# Builds the release executable and the per-user MSI installer into target\wix.
# Needs the repo's local WiX tool: `dotnet tool restore` (see dotnet-tools.json).

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

cargo build --release -p archi-games
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# The workspace version, e.g. 0.1.0.
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$exe = Join-Path $root "target\release\archi-games.exe"
# build.rs writes the icon into the build script's output directory.
$icon = Get-ChildItem "target\release\build\archi-games-*\out\archi-games.ico" | Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $icon) { throw "archi-games.ico not found: did build.rs run?" }

dotnet tool restore | Out-Null
$out = Join-Path $root "target\wix\ArchiGames-$version.msi"
dotnet tool run wix build wix\main.wxs -arch x64 -d Version=$version -d ExePath=$exe -d IconPath=$($icon.FullName) -o $out
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Installer: $out"
