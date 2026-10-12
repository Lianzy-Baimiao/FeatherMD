param([string]$Version = "0.1.4")
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot
$dist=Join-Path $root 'dist'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
$archive=Join-Path $dist "FeatherMD-$Version-windows-x64.zip"
if(Test-Path -LiteralPath $archive){throw "Archive already exists: $archive"}
$exe=Join-Path $root 'target\release\feathermd.exe'
if(-not (Test-Path -LiteralPath $exe)){throw 'Build a release with the MSVC toolchain first'}
Copy-Item -LiteralPath $exe -Destination (Join-Path $dist 'FeatherMD.exe')
$files=@('LICENSE','THIRD_PARTY_NOTICES.md','README.md')
foreach($name in $files){Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $dist $name)}
$docs=Join-Path $dist 'docs'
New-Item -ItemType Directory -Path $docs -Force | Out-Null
foreach($name in @('reading-light.png','reading-dark.png','reading-outline-hidden.png','welcome-light.png','welcome-dark.png','about-light.png','about-dark.png','typography-light.png','typography-dark.png','file-associations-menu.png','PERFORMANCE.md','ARCHITECTURE.md','FILE_ASSOCIATIONS.md')){Copy-Item -LiteralPath (Join-Path $root "docs\$name") -Destination (Join-Path $docs $name)}
Compress-Archive -LiteralPath @((Join-Path $dist 'FeatherMD.exe'),(Join-Path $dist 'LICENSE'),(Join-Path $dist 'THIRD_PARTY_NOTICES.md'),(Join-Path $dist 'README.md'),$docs) -DestinationPath $archive
$hashes=foreach($path in @((Join-Path $dist 'FeatherMD.exe'),$archive)){$hash=Get-FileHash -LiteralPath $path -Algorithm SHA256;"$($hash.Hash.ToLower())  $([IO.Path]::GetFileName($path))"}
$hashes | Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Encoding ascii
Get-ChildItem -LiteralPath $dist | Select-Object Name,Length
