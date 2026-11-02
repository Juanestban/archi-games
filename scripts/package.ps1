# Builds the release and everything a GitHub release publishes, into target\dist:
#   ArchiGames-<version>-x64.msi            per-user installer
#   ArchiGames-<version>-x64-portable.exe   standalone executable
#   SHA256SUMS.txt                          checksums the in-app updater verifies (ADR 0002)
# Needs the repo's local WiX tool (`dotnet tool restore`, see dotnet-tools.json).

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

cargo build --release --locked -p archi-games
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# The workspace version, e.g. 0.1.0.
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$exe = Join-Path $root "target\release\archi-games.exe"
# build.rs writes the icon into the build script's output directory.
$icon = Get-ChildItem "target\release\build\archi-games-*\out\archi-games.ico" | Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $icon) { throw "archi-games.ico not found: did build.rs run?" }

$dist = Join-Path $root "target\dist"
Remove-Item $dist -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $dist | Out-Null
$msi = Join-Path $dist "ArchiGames-$version-x64.msi"
$portable = Join-Path $dist "ArchiGames-$version-x64-portable.exe"

dotnet tool restore | Out-Null
dotnet tool run wix build wix\main.wxs -arch x64 -d Version=$version -d ExePath=$exe -d IconPath=$($icon.FullName) -o $msi
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Remove-Item (Join-Path $dist "*.wixpdb") -ErrorAction SilentlyContinue
Copy-Item $exe $portable

# sha256sum format ("<hex>  <name>"), LF line endings.
$sums = Get-ChildItem $dist -File | Sort-Object Name | ForEach-Object { "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)" }
[System.IO.File]::WriteAllText((Join-Path $dist "SHA256SUMS.txt"), (($sums -join "`n") + "`n"))

Get-ChildItem $dist | Select-Object Name, Length | Format-Table -AutoSize
