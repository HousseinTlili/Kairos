<#
.SYNOPSIS
  Makes the local Hermes Agent API server ready for Kairos (M1 Step 2).

.DESCRIPTION
  Idempotent. Ensures Hermes's .env contains API_SERVER_ENABLED=true,
  API_SERVER_HOST=127.0.0.1 and an API_SERVER_KEY of at least 32 hex characters. Only when a
  change is needed does it back up .env, rewrite those three lines, and restart the gateway.
  Then it checks /health and that the key is accepted, and prints the Hermes version and
  approval mode. The key itself is never printed.

  Exit code 0 = Hermes is ready for Kairos; 1 = it is not (the message says why).

.PARAMETER HermesHome
  Hermes home directory. Default: $env:HERMES_HOME, else %LOCALAPPDATA%\hermes.

.PARAMETER ConfigureOnly
  Only check and fix .env. No gateway restart, no network checks. Used by
  scripts/test-setup.ps1 against a throwaway home.
#>
[CmdletBinding()]
param(
  [string]$HermesHome,
  [switch]$ConfigureOnly
)

Set-StrictMode -Version 3.0
$ErrorActionPreference = 'Stop'

$ManagedNames = @('API_SERVER_ENABLED', 'API_SERVER_HOST', 'API_SERVER_KEY')
$DefaultPort = 8642
$HealthTimeoutSeconds = 30

function Write-Step([string]$Message) { Write-Host "  $Message" }
function Write-Ok([string]$Message) { Write-Host "  [ok] $Message" -ForegroundColor Green }
function Write-Warn([string]$Message) { Write-Host "  [warning] $Message" -ForegroundColor Yellow }
function Stop-WithError([string]$Message) {
  Write-Host "  [error] $Message" -ForegroundColor Red
  exit 1
}

function Resolve-HermesHome([string]$Explicit) {
  if ($Explicit) { return $Explicit }
  if ($env:HERMES_HOME) { return $env:HERMES_HOME }
  return (Join-Path $env:LOCALAPPDATA 'hermes')
}

function Resolve-HermesCli([string]$HomeDir) {
  $bundled = Join-Path $HomeDir 'bin\hermes.exe'
  if (Test-Path -LiteralPath $bundled) { return $bundled }
  $onPath = Get-Command hermes -ErrorAction SilentlyContinue
  if ($onPath) { return $onPath.Source }
  return $null
}

# Reads KEY=value pairs the way dotenv does for the cases Hermes's .env uses: optional
# `export `, optional quotes, last occurrence wins.
function Read-EnvValues([string]$Text) {
  $values = @{}
  foreach ($line in ($Text -split "\r?\n")) {
    if ($line -match '^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$') {
      $value = $Matches[2]
      if ($value.Length -ge 2 -and (($value[0] -eq '"' -and $value[-1] -eq '"') -or ($value[0] -eq "'" -and $value[-1] -eq "'"))) {
        $value = $value.Substring(1, $value.Length - 2)
      }
      $values[$Matches[1]] = $value
    }
  }
  return $values
}

function Test-KeyStrong([string]$Key) {
  return ($null -ne $Key) -and ($Key -match '^[0-9a-fA-F]{32,}$')
}

function New-ApiKey {
  $bytes = New-Object byte[] 32
  $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
  try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
  return (($bytes | ForEach-Object { $_.ToString('x2') }) -join '')
}

# Removes every line that sets a managed name and appends one block with the new values.
# All other bytes of the file, including its mixed line endings, are left exactly as they were.
function Update-EnvText([string]$Text, [hashtable]$NewValues) {
  $namePattern = ($ManagedNames -join '|')
  $stripped = [regex]::Replace($Text, "(?m)^[ \t]*(?:export[ \t]+)?(?:$namePattern)[ \t]*=.*(?:\r?\n|\z)", '')
  $newline = if ($Text.Contains("`r`n")) { "`r`n" } else { "`n" }
  if ($stripped.Length -gt 0 -and -not $stripped.EndsWith("`n")) { $stripped += $newline }
  $block = "# Managed by Kairos scripts/setup.ps1$newline" +
    (($ManagedNames | ForEach-Object { "$_=$($NewValues[$_])" }) -join $newline) + $newline
  return $stripped + $block
}

function Write-Utf8NoBom([string]$Path, [string]$Text) {
  [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

# Windows PowerShell 5.1 turns any stderr line from a native command into a terminating error
# while $ErrorActionPreference is 'Stop', so Hermes CLI calls run with 'Continue'.
function Invoke-Hermes([string]$Exe, [string[]]$Arguments) {
  $saved = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try {
    $output = & $Exe @Arguments 2>$null
    return [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = @($output) }
  } finally {
    $ErrorActionPreference = $saved
  }
}

function Wait-Health([string]$BaseUrl) {
  $deadline = (Get-Date).AddSeconds($HealthTimeoutSeconds)
  while ((Get-Date) -lt $deadline) {
    try {
      $health = Invoke-RestMethod -Uri "$BaseUrl/health" -TimeoutSec 3
      if ($health.status -eq 'ok') { return $health }
    } catch {
      # Not up yet; keep polling until the deadline.
    }
    Start-Sleep -Milliseconds 750
  }
  return $null
}

function Test-KeyAccepted([string]$BaseUrl, [string]$Key) {
  try {
    $null = Invoke-RestMethod -Uri "$BaseUrl/v1/capabilities" -TimeoutSec 5 `
      -Headers @{ Authorization = "Bearer $Key" }
    return $true
  } catch {
    return $false
  }
}

# --- 1. Hermes home -------------------------------------------------------------------------

Write-Host 'Kairos setup: Hermes API server'
$homeDir = Resolve-HermesHome $HermesHome
$configPath = Join-Path $homeDir 'config.yaml'
$envPath = Join-Path $homeDir '.env'

if (-not (Test-Path -LiteralPath $configPath)) {
  Stop-WithError "No Hermes config at $configPath. Install Hermes, or pass -HermesHome."
}
Write-Ok "Hermes home: $homeDir"

# --- 2. .env --------------------------------------------------------------------------------

$envText = ''
if (Test-Path -LiteralPath $envPath) {
  $envText = [System.IO.File]::ReadAllText($envPath)
}
$current = Read-EnvValues $envText

$reasons = @()
if ($current['API_SERVER_ENABLED'] -ne 'true') { $reasons += 'API_SERVER_ENABLED is not true' }
if ($current['API_SERVER_HOST'] -ne '127.0.0.1') { $reasons += 'API_SERVER_HOST is not 127.0.0.1' }
$key = $current['API_SERVER_KEY']
if (-not (Test-KeyStrong $key)) {
  $reasons += 'API_SERVER_KEY is missing or shorter than 32 hex characters'
  $key = New-ApiKey
}

$changed = $reasons.Count -gt 0
if ($changed) {
  foreach ($r in $reasons) { Write-Step "Needs change: $r" }
  if (Test-Path -LiteralPath $envPath) {
    $backup = "$envPath.bak-kairos-$(Get-Date -Format 'yyyyMMddHHmmss')"
    Copy-Item -LiteralPath $envPath -Destination $backup
    Write-Ok "Backed up .env to $backup"
  }
  $newText = Update-EnvText $envText @{
    API_SERVER_ENABLED = 'true'
    API_SERVER_HOST    = '127.0.0.1'
    API_SERVER_KEY     = $key
  }
  Write-Utf8NoBom $envPath $newText
  Write-Ok '.env updated (key not shown)'
} else {
  Write-Ok '.env already configured; nothing changed'
}

if ($ConfigureOnly) { exit 0 }

# --- 3. Gateway and health ------------------------------------------------------------------

$hermes = Resolve-HermesCli $homeDir
if (-not $hermes) { Stop-WithError 'Hermes CLI not found (looked in <home>\bin\hermes.exe and PATH).' }

$port = $DefaultPort
if ($current['API_SERVER_PORT'] -match '^\d+$') { $port = [int]$current['API_SERVER_PORT'] }
$baseUrl = "http://127.0.0.1:$port"

if ($changed) {
  Write-Step 'Restarting the Hermes gateway to apply the change...'
  $env:HERMES_HOME = $homeDir
  $restart = Invoke-Hermes $hermes @('gateway', 'restart')
  if ($restart.ExitCode -ne 0) { Stop-WithError "'hermes gateway restart' failed (exit $($restart.ExitCode))." }
}

$health = Wait-Health $baseUrl
if (-not $health) {
  Stop-WithError "Hermes API server not answering at $baseUrl/health after $HealthTimeoutSeconds s. Start it with: hermes gateway restart"
}
Write-Ok "Hermes $($health.version) answering at $baseUrl"

if (-not (Test-KeyAccepted $baseUrl $key)) {
  Stop-WithError 'Hermes rejected the key in .env. If you just edited .env by hand, run: hermes gateway restart'
}
Write-Ok 'API key accepted'

# --- 4. Approval mode (D-009) ---------------------------------------------------------------

$env:HERMES_HOME = $homeDir
$modeResult = Invoke-Hermes $hermes @('config', 'get', 'approvals.mode')
$mode = ''
if ($modeResult.Output.Count -gt 0) { $mode = ([string]$modeResult.Output[-1]).Trim() }
if ($mode -eq 'smart' -or $mode -eq 'manual') {
  Write-Ok "Approval mode: $mode"
} else {
  Write-Warn "Approval mode is '$mode', which can run commands without asking. Kairos expects 'smart' (with its rules) or 'manual' (docs/DECISIONS.md D-009). Set it with: hermes config set approvals.mode smart"
}

Write-Host 'Hermes is ready for Kairos.'
exit 0
