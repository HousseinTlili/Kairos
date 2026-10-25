<#
.SYNOPSIS
  Makes Hermes's gateway start when you log in to Windows (M3 Step 7, D-013).

.DESCRIPTION
  Runs `hermes gateway install`, Hermes's own login start: a per-user Scheduled Task named
  Hermes_Gateway (no admin rights, 30 s after logon, restarted if it fails), or an entry in your
  Startup folder when Task Scheduler refuses. Hermes asks its own questions ("start now?",
  "start on login?"); answer yes to the second.

  It doesn't touch Hermes's .env or config.yaml. Kairos never runs this by itself; Settings ▸
  Startup only shows whether the task exists.

  -Status only reports what is installed. -Uninstall runs `hermes gateway uninstall`.

  Exit code 0 = done; 1 = not done (the message says why).
#>
[CmdletBinding()]
param(
  [string]$HermesHome,
  [switch]$Yes,
  [switch]$Status,
  [switch]$Uninstall
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

# The names Hermes uses for the default profile (hermes_cli/gateway_windows.py).
$taskName = 'Hermes_Gateway'
$startupEntry = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Startup\$taskName.vbs"

function Get-AutostartState {
  $saved = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try {
    & schtasks.exe /Query /TN $taskName 2>&1 | Out-Null
    $task = ($LASTEXITCODE -eq 0)
  } finally {
    $ErrorActionPreference = $saved
  }
  return [pscustomobject]@{ Task = $task; StartupEntry = (Test-Path -LiteralPath $startupEntry) }
}

function Write-State($State) {
  if ($State.Task) { Write-Ok "Scheduled Task '$taskName' exists: the gateway starts when you log in" }
  elseif ($State.StartupEntry) { Write-Ok "Startup-folder entry exists: the gateway starts when you log in" }
  else { Write-Step "The gateway does not start by itself at login yet." }
}

Write-Host 'Hermes gateway at login'
$before = Get-AutostartState

if ($Status) {
  Write-State $before
  exit 0
}

if ($Uninstall) {
  Confirm-Change 'Stop the Hermes gateway from starting at login (hermes gateway uninstall)?'
  & $hermes gateway uninstall
  if ($LASTEXITCODE -ne 0) { Stop-WithError "'hermes gateway uninstall' failed (exit $LASTEXITCODE)." }
  Write-State (Get-AutostartState)
  exit 0
}

if ($before.Task -or $before.StartupEntry) {
  Write-State $before
  Write-Ok 'Nothing to do.'
  exit 0
}

Write-Step "Will run 'hermes gateway install' (a per-user Scheduled Task; no admin rights)."
Write-Step 'Hermes will ask whether to start now and whether to start at login: answer yes to login.'
Confirm-Change 'Go ahead?'

# Interactive on purpose: Hermes's own prompts go straight to this terminal.
& $hermes gateway install
if ($LASTEXITCODE -ne 0) { Stop-WithError "'hermes gateway install' failed (exit $LASTEXITCODE)." }

$after = Get-AutostartState
Write-State $after
if (-not ($after.Task -or $after.StartupEntry)) {
  Stop-WithError 'Hermes finished, but no login start was found. Did you answer no to "start on login"?'
}
exit 0
