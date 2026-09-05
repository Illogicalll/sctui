# sctui installer for Windows — https://github.com/Illogicalll/sctui
#
#   irm https://raw.githubusercontent.com/Illogicalll/sctui/main/install.ps1 | iex
#
# Environment:
#   $env:SCTUI_VERSION = "v0.1.0"        install a specific release (default: latest)
#   $env:SCTUI_INSTALL_DIR = "C:\..."    where to put sctui.exe (default: %LOCALAPPDATA%\sctui\bin)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$repo = "Illogicalll/sctui"
$target = "x86_64-pc-windows-msvc"

if (-not [Environment]::Is64BitOperatingSystem) { throw "sctui needs 64-bit Windows" }
if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
    Write-Host "note: no native ARM64 build yet; installing the x86_64 build, which runs under emulation."
}

$installDir = if ($env:SCTUI_INSTALL_DIR) { $env:SCTUI_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "sctui\bin" }

$version = $env:SCTUI_VERSION
if (-not $version) {
    $latest = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers @{ "User-Agent" = "sctui-installer" }
    $version = $latest.tag_name
}

$name = "sctui-$target"
$base = "https://github.com/$repo/releases/download/$version"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("sctui-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    Write-Host "Downloading sctui $version for $target..."
    Invoke-WebRequest "$base/$name.zip" -OutFile "$tmp\$name.zip"
    Invoke-WebRequest "$base/$name.zip.sha256" -OutFile "$tmp\$name.zip.sha256"

    $expected = ((Get-Content "$tmp\$name.zip.sha256" -Raw) -split "\s+")[0].ToLower()
    $actual = (Get-FileHash "$tmp\$name.zip" -Algorithm SHA256).Hash.ToLower()
    if ($expected -ne $actual) { throw "checksum mismatch for $name.zip" }

    Expand-Archive "$tmp\$name.zip" -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item "$tmp\$name\sctui.exe" (Join-Path $installDir "sctui.exe") -Force
    Write-Host "Installed sctui $version to $installDir\sctui.exe"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (($userPath -split ";") -notcontains $installDir) {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$installDir", "User")
        $env:Path = "$env:Path;$installDir"
        Write-Host "Added $installDir to your user PATH. Open a new terminal to pick it up."
    }

    Write-Host "Run 'sctui' to start. The first launch opens your browser to sign in to SoundCloud."
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
