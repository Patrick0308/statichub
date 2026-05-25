$ErrorActionPreference = 'Stop'

param(
  [Parameter(Position=0)]
  [ValidateSet('cli','server','both')]
  [string]$Scope = 'cli',

  [string]$Version = 'latest',

  [string]$BinDir = "$env:LOCALAPPDATA\statichub\bin"
)

function Show-Usage {
  @"
StaticHub installer (Windows PowerShell)

Usage:
  irm https://raw.githubusercontent.com/Patrick0308/statichub/main/install.ps1 | iex

Direct script usage with params:
  .\install.ps1
  .\install.ps1 server
  .\install.ps1 both
  .\install.ps1 -Scope both -Version v0.1.0-beta.12 -BinDir "`$env:LOCALAPPDATA\statichub\bin"

Scope values:
  cli    Install statichub only (default)
  server Install statichub-server only
  both   Install both binaries
"@
}

if ($args -contains '-h' -or $args -contains '--help') {
  Show-Usage
  exit 0
}

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne 'AMD64' -and $arch -ne 'x86_64') {
  throw "Unsupported Windows architecture: $arch. Currently only x86_64/AMD64 is supported."
}

$target = 'x86_64-windows'
$repo = 'Patrick0308/statichub'
$zipUrl = if ($Version -eq 'latest') {
  "https://github.com/$repo/releases/latest/download/statichub-$target.zip"
} else {
  "https://github.com/$repo/releases/download/$Version/statichub-$target.zip"
}

$tempRoot = Join-Path $env:TEMP ("statichub-install-" + [guid]::NewGuid().ToString('N'))
$tempZip = Join-Path $tempRoot 'statichub.zip'
$tempExtract = Join-Path $tempRoot 'extract'
New-Item -ItemType Directory -Path $tempExtract -Force | Out-Null
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null

try {
  Write-Host "Downloading: $zipUrl"
  Invoke-WebRequest -Uri $zipUrl -OutFile $tempZip
  Expand-Archive -Path $tempZip -DestinationPath $tempExtract -Force

  function Install-Bin([string]$fileName) {
    $src = Join-Path $tempExtract $fileName
    if (-not (Test-Path $src)) {
      throw "Expected binary not found in archive: $fileName"
    }
    $dst = Join-Path $BinDir $fileName
    Copy-Item -Path $src -Destination $dst -Force
    Write-Host "Installed $fileName -> $dst"
  }

  switch ($Scope) {
    'cli'    { Install-Bin 'statichub.exe' }
    'server' { Install-Bin 'statichub-server.exe' }
    'both'   {
      Install-Bin 'statichub.exe'
      Install-Bin 'statichub-server.exe'
    }
  }

  $pathEntries = [Environment]::GetEnvironmentVariable('Path', 'User') -split ';'
  if ($pathEntries -notcontains $BinDir) {
    $newUserPath = ([Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $BinDir).Trim(';')
    [Environment]::SetEnvironmentVariable('Path', $newUserPath, 'User')
    Write-Host ""
    Write-Host "Added to your USER PATH: $BinDir"
    Write-Host "Open a new terminal to use the command."
  }

  Write-Host ""
  Write-Host "Done. Verify with:"
  switch ($Scope) {
    'cli'    { Write-Host '  statichub version' }
    'server' { Write-Host '  statichub-server version' }
    'both'   {
      Write-Host '  statichub version'
      Write-Host '  statichub-server version'
    }
  }
}
catch {
  Write-Error "Install failed: $($_.Exception.Message)"
  Write-Host "Release assets: https://github.com/$repo/releases"
  exit 1
}
finally {
  if (Test-Path $tempRoot) {
    Remove-Item -Path $tempRoot -Recurse -Force
  }
}
