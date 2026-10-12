param([string]$Directory="$PSScriptRoot\..\test-artifacts\fixtures")
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Path $Directory -Force | Out-Null
$paragraph="# FeatherMD Rust performance fixture`n`n## Markdown at scale`n`nA UTF-8 paragraph with **bold**, *emphasis*, 中文字符 and a [link](https://example.com).`n`n- List one`n- List two`n`n"
$bytes=[Text.Encoding]::UTF8.GetBytes(($paragraph*7000))
foreach($size in @(1MB,64MB,1GB)){
 $path=Join-Path $Directory "$size.md"
 if(Test-Path -LiteralPath $path){if((Get-Item -LiteralPath $path).Length -eq $size){continue};throw "Refusing to overwrite $path"}
 $file=[IO.File]::Create($path)
 try{$remaining=$size;while($remaining -gt 0){$n=[Math]::Min($bytes.Length,$remaining);$file.Write($bytes,0,$n);$remaining-=$n}}finally{$file.Dispose()}
 Get-Item -LiteralPath $path | Select-Object Name,Length
}
