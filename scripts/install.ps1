# Installs the latest candlerail release on Windows.
#
#   powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/upendrx/candlerail/main/scripts/install.ps1 | iex"
#
# Environment:
#   CANDLERAIL_INSTALL_DIR  where to put the binary (default: %LOCALAPPDATA%\candlerail)
#   CANDLERAIL_VERSION      a release tag such as v0.2.0 (default: latest)
$ErrorActionPreference = "Stop"

$repo = "upendrx/candlerail"
$target = "x86_64-pc-windows-msvc"
$dir = if ($env:CANDLERAIL_INSTALL_DIR) { $env:CANDLERAIL_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "candlerail" }
$version = if ($env:CANDLERAIL_VERSION) { $env:CANDLERAIL_VERSION } else { "latest" }

$url = if ($version -eq "latest") {
    "https://github.com/$repo/releases/latest/download/candlerail-$target.zip"
} else {
    "https://github.com/$repo/releases/download/$version/candlerail-$target.zip"
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("candlerail-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Host "Downloading candlerail ($target)..."
    $zip = Join-Path $tmp "candlerail.zip"
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    Expand-Archive -Path $zip -DestinationPath $tmp -Force

    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    Copy-Item (Join-Path $tmp "candlerail-$target\candlerail.exe") (Join-Path $dir "candlerail.exe") -Force
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

$exe = Join-Path $dir "candlerail.exe"
Write-Host "Installed $(& $exe --version) to $exe"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not (($userPath -split ";") -contains $dir)) {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$dir", "User")
    $env:Path = "$env:Path;$dir"
    Write-Host "Added $dir to your PATH. Open a new terminal for it to take effect everywhere."
}

Write-Host ""
Write-Host "Start it with:"
Write-Host "  candlerail serve"
Write-Host "then open http://127.0.0.1:8787 in your browser."
