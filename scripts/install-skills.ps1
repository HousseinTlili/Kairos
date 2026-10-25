<#
.SYNOPSIS
  Installs the Kairos skills into Hermes (M5 Step 6), or removes them with -Uninstall.

.DESCRIPTION
  Links %HERMES_HOME%\skills\kairos to this repository's skills\kairos folder (a directory
  junction, like the plugin), so Hermes lists kairos-debug-and-fix, kairos-code-review and
  kairos-documents-to-actions from the next conversation, each also as a slash command.

  It adds files to Hermes's skills folder, so it asks first unless -Yes is given. It changes no
  Hermes configuration.

  -Uninstall removes the link (never the repository's files).

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

function Confirm-Change([string]$Question) {
  if ($Yes) { return }
  $answer = Read-Host "  $Question [y/N]"
  if ($answer -notmatch '^(y|yes)$') { Stop-WithError 'Cancelled; nothing was changed.' }
}

function Get-LinkTarget([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
  if ($null -eq $item) { return $null }
  if ($item.LinkType -eq 'Junction' -or $item.LinkType -eq 'SymbolicLink') { return @($item.Target)[0] }
  return ''  # a real folder, not a link
}

$repo = Split-Path -Parent $PSScriptRoot
$source = Join-Path $repo 'skills\kairos'
if (-not $HermesHome) {
  $HermesHome = if ($env:HERMES_HOME) { $env:HERMES_HOME } else { Join-Path $env:LOCALAPPDATA 'hermes' }
}
$skillsDir = Join-Path $HermesHome 'skills'
$link = Join-Path $skillsDir 'kairos'

Write-Host 'Kairos skills'

if ($Uninstall) {
  $target = Get-LinkTarget $link
  if ($null -eq $target) { Write-Ok 'Not installed; nothing to do.'; exit 0 }
  if ($target -eq '') { Stop-WithError "$link is a real folder, not the Kairos link; left it alone." }
  Confirm-Change "Remove the link $link?"
  # Removing a junction removes the link only, never the repository files it points to.
  [System.IO.Directory]::Delete($link)
  Write-Ok "Removed $link"
  exit 0
}

if (-not (Test-Path -LiteralPath $source)) { Stop-WithError "Skills not found at $source." }
$sourceFull = (Resolve-Path -LiteralPath $source).Path.TrimEnd('\')

$target = Get-LinkTarget $link
if ($null -ne $target) {
  if ($target -eq '') { Stop-WithError "$link already exists as a real folder. Remove it yourself if it is an old copy." }
  if ($target.TrimEnd('\') -ieq $sourceFull) { Write-Ok "Already linked: $link -> $sourceFull"; exit 0 }
  Stop-WithError "$link links to $target, not this repository. Run with -Uninstall from that checkout first."
}

Write-Step "Will link $link -> $sourceFull"
foreach ($skill in (Get-ChildItem -LiteralPath $source -Directory)) { Write-Step "  $($skill.Name)" }
Confirm-Change 'Go ahead?'

New-Item -ItemType Directory -Force -Path $skillsDir | Out-Null
New-Item -ItemType Junction -Path $link -Target $sourceFull | Out-Null
Write-Ok "Linked $link -> $sourceFull"
Write-Step 'Hermes lists them from the next conversation (try /kairos-code-review).'
exit 0
