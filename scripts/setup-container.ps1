<#
.SYNOPSIS
  Runs the agent's commands in Docker, one fresh container per conversation (M7 Step 2, D-019).

.DESCRIPTION
  Uses Hermes's own Docker terminal backend; it only changes Hermes settings through the Hermes CLI
  (never by editing config.yaml), and asks first unless -Yes is given:

    hermes config set terminal.backend docker
    hermes config set terminal.container_persistent false      # a fresh container per conversation
    hermes config set terminal.docker_mount_cwd_to_workspace false
    hermes config set terminal.docker_volumes '["C:/Users/you/Projects:/workspace/projects/Projects"]'

  Only the folders you choose are visible inside the container (read-write), under
  /workspace/projects/<folder name>. Nothing else of the PC is.

  Needs Docker Desktop running with the WSL2 engine.

  -Status shows the current backend and mounts. -Undo sets terminal.backend back to local.
  -Folders "C:\a","D:\b" chooses the folders without the prompt.

  Exit code 0 = done; 1 = not done (the message says why).
#>
[CmdletBinding()]
param(
  [string]$HermesHome,
  [string[]]$Folders,
  [switch]$Yes,
  [switch]$Status,
  [switch]$Undo
)

Set-StrictMode -Version 3.0
$ErrorActionPreference = 'Stop'

function Write-Ok([string]$Message) { Write-Host "  [ok] $Message" -ForegroundColor Green }
function Write-Step([string]$Message) { Write-Host "  $Message" }
function Stop-WithError([string]$Message) {
  Write-Host "  [error] $Message" -ForegroundColor Red
  exit 1
}

function Confirm-Change([string]$Question) {
  if ($Yes) { return }
  $answer = Read-Host "  $Question [y/N]"
  if ($answer -notmatch '^(y|yes)$') { Stop-WithError 'Cancelled; nothing was changed.' }
}

# Windows PowerShell 5.1 turns native stderr into terminating errors under 'Stop' (see setup.ps1).
function Invoke-Native([string]$Exe, [string[]]$Arguments) {
  $saved = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try {
    $output = & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
    return [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = @($output) }
  } finally {
    $ErrorActionPreference = $saved
  }
}

if (-not $HermesHome) {
  $HermesHome = if ($env:HERMES_HOME) { $env:HERMES_HOME } else { Join-Path $env:LOCALAPPDATA 'hermes' }
}
$hermes = Join-Path $HermesHome 'bin\hermes.exe'
if (-not (Test-Path -LiteralPath $hermes)) {
  $onPath = Get-Command hermes -ErrorAction SilentlyContinue
  if (-not $onPath) { Stop-WithError 'Hermes CLI not found.' }
  $hermes = $onPath.Source
}
$env:HERMES_HOME = $HermesHome

function Get-HermesSetting([string]$Key) {
  $result = Invoke-Native $hermes @('config', 'get', $Key)
  if ($result.ExitCode -ne 0) { return $null }
  return (@($result.Output) | Where-Object { $_.Trim() } | Select-Object -Last 1)
}

function Set-HermesSetting([string]$Key, [string]$Value) {
  $result = Invoke-Native $hermes @('config', 'set', $Key, $Value)
  if ($result.ExitCode -ne 0) { Stop-WithError "'hermes config set $Key' failed: $($result.Output -join ' ')" }
  Write-Ok "$Key = $Value"
}

Write-Host 'Kairos container (Hermes Docker backend)'

if ($Status) {
  Write-Step "terminal.backend: $(Get-HermesSetting 'terminal.backend')"
  Write-Step "terminal.container_persistent: $(Get-HermesSetting 'terminal.container_persistent')"
  Write-Step "terminal.docker_volumes: $(Get-HermesSetting 'terminal.docker_volumes')"
  exit 0
}

if ($Undo) {
  Confirm-Change 'Run the agent''s commands directly on this PC again (terminal.backend local)?'
  Set-HermesSetting 'terminal.backend' 'local'
  Write-Step 'Restart the Hermes gateway for it to apply to new conversations.'
  exit 0
}

# --- 1. Docker ---------------------------------------------------------------------------------

$docker = Get-Command docker -ErrorAction SilentlyContinue
if (-not $docker) { Stop-WithError 'Docker is not installed. Install Docker Desktop (WSL2 engine) first.' }
$version = Invoke-Native $docker.Source @('version', '--format', '{{.Server.Version}}')
if ($version.ExitCode -ne 0) { Stop-WithError 'Docker is installed but not running. Start Docker Desktop and try again.' }
Write-Ok "Docker engine $(@($version.Output)[-1])"

# --- 2. Folders --------------------------------------------------------------------------------

if (-not $Folders) {
  $default = Join-Path $env:USERPROFILE 'Projects'
  Write-Step 'Which folders should the agent see? (comma-separated; everything else stays hidden)'
  $suggest = if (Test-Path -LiteralPath $default) { $default } else { '' }
  $answer = Read-Host "  Folders [$suggest]"
  if (-not $answer) { $answer = $suggest }
  $Folders = @($answer -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}
if (-not $Folders -or $Folders.Count -eq 0) { Stop-WithError 'No folder chosen; nothing was changed.' }

$volumes = @()
foreach ($folder in $Folders) {
  if (-not (Test-Path -LiteralPath $folder -PathType Container)) { Stop-WithError "Not a folder: $folder" }
  $full = (Resolve-Path -LiteralPath $folder).Path.TrimEnd('\')
  $name = Split-Path -Leaf $full
  # Docker Desktop takes Windows paths with forward slashes.
  $volumes += ('{0}:/workspace/projects/{1}' -f ($full -replace '\\', '/'), $name)
}
$volumesJson = ConvertTo-Json -InputObject @($volumes) -Compress

Write-Step 'Will set, through the Hermes CLI:'
Write-Step '  terminal.backend = docker'
Write-Step '  terminal.container_persistent = false   (a fresh container per conversation)'
Write-Step '  terminal.docker_mount_cwd_to_workspace = false'
Write-Step "  terminal.docker_volumes = $volumesJson"
Confirm-Change 'Go ahead?'

Set-HermesSetting 'terminal.container_persistent' 'false'
Set-HermesSetting 'terminal.docker_mount_cwd_to_workspace' 'false'
Set-HermesSetting 'terminal.docker_volumes' $volumesJson
Set-HermesSetting 'terminal.backend' 'docker'
Write-Step 'Restart the Hermes gateway (in your own terminal) for new conversations to use the container.'
Write-Step 'Check that Hermes stored the folders as a list:'
Write-Step "  terminal.docker_volumes: $(Get-HermesSetting 'terminal.docker_volumes')"
Write-Step 'Undo any time: scripts/setup-container.ps1 -Undo'
exit 0
