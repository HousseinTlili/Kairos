<#
.SYNOPSIS
  Installs the Kairos plugin into Hermes (M2 Step 5), or removes it with -Uninstall.

.DESCRIPTION
  1. Links %HERMES_HOME%\plugins\kairos to this repository's plugin\kairos folder (a directory
     junction, so code changes apply on the next gateway restart, without copying).
  2. Writes the starter rules to %APPDATA%\kairos\policy.json if no rules file exists yet. An
     existing rules file is never overwritten.
  3. Runs `hermes plugins enable kairos`, which loads the plugin into the running gateway.

  It changes Hermes's plugin list, so it asks first unless -Yes is given. It never restarts the
  gateway (CLAUDE.md: start Hermes from a terminal the user keeps open).

  -Uninstall runs `hermes plugins disable kairos` and removes the link. The rules file and the
  audit log in %APPDATA%\kairos are kept.

  Exit code 0 = done; 1 = not done (the message says why).
#>
[CmdletBinding()]
param(
  [string]$HermesHome,
  [switch]$Yes,
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

# Windows PowerShell 5.1 turns native stderr into terminating errors under 'Stop' (see setup.ps1).
function Invoke-Hermes([string]$Exe, [string[]]$Arguments) {
  $saved = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try {
    $output = & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
    return [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = @($output) }
  } finally {
    $ErrorActionPreference = $saved
  }
}

function Confirm-Change([string]$Question) {
  if ($Yes) { return }
  $answer = Read-Host "  $Question [y/N]"
  if ($answer -notmatch '^(y|yes)$') { Stop-WithError 'Cancelled; nothing was changed.' }
}

$repo = Split-Path -Parent $PSScriptRoot
$source = Join-Path $repo 'plugin\kairos'
if (-not $HermesHome) {
  $HermesHome = if ($env:HERMES_HOME) { $env:HERMES_HOME } else { Join-Path $env:LOCALAPPDATA 'hermes' }
}
$pluginsDir = Join-Path $HermesHome 'plugins'
$link = Join-Path $pluginsDir 'kairos'
$kairosHome = Join-Path $env:APPDATA 'kairos'
$policy = Join-Path $kairosHome 'policy.json'

$hermes = Join-Path $HermesHome 'bin\hermes.exe'
if (-not (Test-Path -LiteralPath $hermes)) {
  $onPath = Get-Command hermes -ErrorAction SilentlyContinue
  if (-not $onPath) { Stop-WithError 'Hermes CLI not found.' }
  $hermes = $onPath.Source
}
$env:HERMES_HOME = $HermesHome

function Get-LinkTarget([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
  if ($null -eq $item) { return $null }
  if ($item.LinkType -eq 'Junction' -or $item.LinkType -eq 'SymbolicLink') { return @($item.Target)[0] }
  return ''  # a real folder, not a link
}

Write-Host 'Kairos plugin'

if ($Uninstall) {
  Confirm-Change "Disable the Kairos plugin in Hermes and remove $link?"
  $result = Invoke-Hermes $hermes @('plugins', 'disable', 'kairos')
  if ($result.ExitCode -ne 0) { Stop-WithError "'hermes plugins disable kairos' failed: $($result.Output -join ' ')" }
  Write-Ok 'Disabled in Hermes (loaded hooks stay until the gateway restarts)'
  $target = Get-LinkTarget $link
  if ($null -ne $target) {
    if ($target -eq '') { Stop-WithError "$link is a real folder, not the Kairos link; left it alone." }
    # Removing a junction removes the link only, never the repository files it points to.
    [System.IO.Directory]::Delete($link)
    Write-Ok "Removed the link $link"
  }
  Write-Ok "Kept $kairosHome (rules and audit log)"
  exit 0
}

if (-not (Test-Path -LiteralPath (Join-Path $source '__init__.py'))) {
  Stop-WithError "Plugin source not found at $source."
}

# --- 1. Link ---------------------------------------------------------------------------------

$target = Get-LinkTarget $link
$sourceFull = (Resolve-Path -LiteralPath $source).Path.TrimEnd('\')
$needsLink = $true
if ($null -ne $target) {
  if ($target -eq '') { Stop-WithError "$link already exists as a real folder. Remove it yourself if it is an old copy." }
  if ($target.TrimEnd('\') -ieq $sourceFull) {
    Write-Ok "Already linked: $link -> $sourceFull"
    $needsLink = $false
  } else {
    Stop-WithError "$link links to $target, not this repository. Run with -Uninstall from that checkout first."
  }
}

# --- 2. Rules --------------------------------------------------------------------------------

$needsPolicy = -not (Test-Path -LiteralPath $policy)

$changes = @()
if ($needsLink) { $changes += "link $link -> $sourceFull" }
if ($needsPolicy) { $changes += "write the starter rules to $policy" }
$changes += "run 'hermes plugins enable kairos' (loads the plugin into the running gateway)"
foreach ($c in $changes) { Write-Step "Will $c" }
Confirm-Change 'Go ahead?'

if ($needsLink) {
  New-Item -ItemType Directory -Force -Path $pluginsDir | Out-Null
  New-Item -ItemType Junction -Path $link -Target $sourceFull | Out-Null
  Write-Ok "Linked $link -> $sourceFull"
}

if ($needsPolicy) {
  New-Item -ItemType Directory -Force -Path $kairosHome | Out-Null
  Copy-Item -LiteralPath (Join-Path $source 'policy.default.json') -Destination $policy
  Write-Ok "Wrote the starter rules to $policy"
} else {
  Write-Ok "Kept the existing rules in $policy"
}

# --- 3. Enable -------------------------------------------------------------------------------

$result = Invoke-Hermes $hermes @('plugins', 'enable', 'kairos')
if ($result.ExitCode -ne 0) { Stop-WithError "'hermes plugins enable kairos' failed: $($result.Output -join ' ')" }
foreach ($line in $result.Output) { if ($line.Trim()) { Write-Step "hermes: $($line.Trim())" } }

$list = Invoke-Hermes $hermes @('plugins', 'list', '--plain', '--no-bundled')
$row = $list.Output | Where-Object { $_ -match '\bkairos\b' -and $_ -notmatch 'kairos-' } | Select-Object -First 1
if (-not $row) { Stop-WithError "Hermes doesn't list the kairos plugin. Output: $($list.Output -join ' | ')" }
Write-Ok "Hermes lists it: $($row.Trim())"
Write-Host 'Kairos plugin installed. Hermes now checks every tool call against your Kairos rules.'
exit 0
