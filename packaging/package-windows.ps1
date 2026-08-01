$ErrorActionPreference = 'Stop'
$version = if ($env:PINORA_VERSION) { $env:PINORA_VERSION } else { (Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value }
$out = if ($env:PINORA_OUTPUT_DIR) { $env:PINORA_OUTPUT_DIR } else { 'target/package' }
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $out, 'target/package-stage'
New-Item -ItemType Directory -Force $out, 'target/package-stage/windows' | Out-Null
cargo build --release --locked
$binary = Join-Path (Get-Location) 'target/release/pinora.exe'
if (-not (Test-Path $binary)) { throw "missing release binary: $binary" }
$stage = Join-Path (Get-Location) 'target/package-stage/windows'
Copy-Item $binary (Join-Path $stage 'pinora.exe')
Compress-Archive -Path (Join-Path $stage 'pinora.exe') -DestinationPath (Join-Path $out "pinora-$version-windows-x86_64.zip") -Force

$makensis = Get-Command makensis -ErrorAction SilentlyContinue
if (-not $makensis -and (Test-Path 'C:\Program Files (x86)\NSIS\makensis.exe')) { $makensis = Get-Item 'C:\Program Files (x86)\NSIS\makensis.exe' }
if ($makensis) {
  & $makensis.Source "/DVERSION=$version" "/DOUTFILE=$(Join-Path (Get-Location) "$out\pinora-$version-windows-x86_64-setup.exe")" packaging/pinora.nsi
  if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }
}

Get-ChildItem $out -File | Where-Object Name -ne 'SHA256SUMS.txt' | Get-FileHash -Algorithm SHA256 |
  ForEach-Object { "{0}  {1}" -f $_.Hash.ToLowerInvariant(), $_.Path.Substring((Resolve-Path $out).Path.Length + 1) } |
  Set-Content (Join-Path $out 'SHA256SUMS.txt')
Write-Host "Packaged Pinora $version for windows/x86_64 in $out"
