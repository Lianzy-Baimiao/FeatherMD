param([string]$Version = "0.1.11", [string]$Output = "")
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot
$dist=if($Output){[IO.Path]::GetFullPath($Output)}else{Join-Path $root 'dist'}
New-Item -ItemType Directory -Path $dist -Force | Out-Null
if((Get-Item -LiteralPath $dist).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Refusing a linked package directory'}
$archive=Join-Path $dist "FeatherMD-$Version-windows-x64.zip"
if(Test-Path -LiteralPath $archive){throw "Archive already exists: $archive"}
$exe=Join-Path $root 'target\release\feathermd.exe'
if(-not (Test-Path -LiteralPath $exe)){throw 'Build a release with the MSVC toolchain first'}
Copy-Item -LiteralPath $exe -Destination (Join-Path $dist 'FeatherMD.exe')
$readmes=@('README.md','README.zh-TW.md','README.en.md','README.ja.md')
$files=@('LICENSE','THIRD_PARTY_NOTICES.md')+$readmes
foreach($name in $files){Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $dist $name)}
$docs=Join-Path $dist 'docs'
# A staging folder: start empty so no file of an earlier package ends up in the ZIP.
$resolvedDocs=[IO.Path]::GetFullPath($docs)
$resolvedDist=[IO.Path]::GetFullPath($dist)
if($resolvedDocs -ne (Join-Path $resolvedDist 'docs')){throw 'Invalid docs staging path'}
if((Test-Path -LiteralPath $docs) -and ((Get-Item -LiteralPath $docs).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Refusing to remove a linked staging folder'}
if(Test-Path -LiteralPath $docs){Remove-Item -LiteralPath $docs -Recurse -Force}
New-Item -ItemType Directory -Path $docs -Force | Out-Null
$shots=@('reading-light.png','reading-dark.png','reading-outline-hidden.png','welcome-light.png','welcome-dark.png','about-light.png','about-dark.png','typography-light.png','typography-dark.png','settings-skins-light.png','settings-skins-dark.png','settings-editor.png','settings-update.png','settings-language.png','settings-reading.png','copy-button.png','reading-table.png','reading-typography.png','reading-skins.png','reading-images.png','preview-selection.png','table-selection.png','source-selection.png','reading-book.png','reading-ancient.png','file-associations-menu.png')
$shots+=@('reading-web-images.png','reading-web-images-loaded.png')
# Each translated README shows its own reading view, About window and language page.
foreach($code in @('zh-TW','en','ja')){$shots+=@("reading-light.$code.png","about-light.$code.png","settings-language.$code.png")}
foreach($name in $shots+@('PERFORMANCE.md','ARCHITECTURE.md','FILE_ASSOCIATIONS.md','PRIVACY.md','sample.md')){Copy-Item -LiteralPath (Join-Path $root "docs\$name") -Destination (Join-Path $docs $name)}
Compress-Archive -LiteralPath (@((Join-Path $dist 'FeatherMD.exe'),(Join-Path $dist 'LICENSE'),(Join-Path $dist 'THIRD_PARTY_NOTICES.md'))+@($readmes | ForEach-Object { Join-Path $dist $_ })+@($docs)) -DestinationPath $archive
$hashes=foreach($path in @((Join-Path $dist 'FeatherMD.exe'),$archive)){$hash=Get-FileHash -LiteralPath $path -Algorithm SHA256;"$($hash.Hash.ToLower())  $([IO.Path]::GetFileName($path))"}
$hashes | Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Encoding ascii
Get-ChildItem -LiteralPath $dist | Select-Object Name,Length
