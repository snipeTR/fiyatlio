# vX.Y.Z etiketini sürüm dosyaları ve CHANGELOG.md ile karşılaştırır.
# Eşleşirse release gövdesini release-notes.md dosyasına yazar.
param(
    [Parameter(Mandatory = $true)]
    [string]$Tag
)

$ErrorActionPreference = "Stop"

if ($Tag -notmatch '^v(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.]+)?)$') {
    throw "Etiket vX.Y.Z biçiminde olmalı. Gelen: $Tag"
}
$version = $Matches[1]

function Read-JsonVersion([string]$Path) {
    $json = Get-Content -Raw -Path $Path | ConvertFrom-Json
    return [string]$json.version
}

$tauri = Read-JsonVersion "src-tauri/tauri.conf.json"
$package = Read-JsonVersion "package.json"
$cargoLine = Select-String -Path "src-tauri/Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $cargoLine) {
    throw "src-tauri/Cargo.toml içinde sürüm satırı yok."
}
$cargo = $cargoLine.Matches[0].Groups[1].Value

$mismatch = @()
if ($tauri -ne $version) { $mismatch += "tauri.conf.json=$tauri" }
if ($package -ne $version) { $mismatch += "package.json=$package" }
if ($cargo -ne $version) { $mismatch += "Cargo.toml=$cargo" }
if ($mismatch.Count -gt 0) {
    throw "Etiket v$version sürüm dosyalarıyla uyuşmuyor: $($mismatch -join ', ')."
}

$utf8 = New-Object System.Text.UTF8Encoding $false
$changelogPath = Join-Path (Get-Location) "CHANGELOG.md"
$changelog = [System.IO.File]::ReadAllText($changelogPath, $utf8)
$pattern = "(?ms)^## \[$([regex]::Escape($version))\b[^\r\n]*\r?\n(.*?)(?=^## |\z)"
$found = [regex]::Match($changelog, $pattern)
if (-not $found.Success) {
    throw "CHANGELOG.md içinde ## [$version] bölümü yok."
}
$body = $found.Groups[1].Value.Trim()
if (-not $body) {
    throw "CHANGELOG.md [$version] bölümü boş."
}

$out = Join-Path (Get-Location) "release-notes.md"
[System.IO.File]::WriteAllText($out, $body + "`n", $utf8)
Write-Output "Sürüm v$version doğrulandı. Sürüm notu: $out"
