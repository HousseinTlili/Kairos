<#
.SYNOPSIS
  Behaviour tests for scripts/setup.ps1. Runs it with -ConfigureOnly against throwaway Hermes
  homes in %TEMP%, so the real Hermes and its .env are never touched.

  Run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-setup.ps1
  Exit code 0 = all tests passed.
#>
Set-StrictMode -Version 3.0
$ErrorActionPreference = 'Stop'

$setup = Join-Path $PSScriptRoot 'setup.ps1'
$script:failures = 0
$script:passed = 0

function New-FakeHome {
  $dir = Join-Path ([System.IO.Path]::GetTempPath()) ("kairos-setup-test-" + [guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Path $dir | Out-Null
  Set-Content -LiteralPath (Join-Path $dir 'config.yaml') -Value 'model: {}' -Encoding ascii
  return $dir
}

function Write-Bytes([string]$Path, [string]$Text) {
  [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Invoke-Setup([string]$HomeDir) {
  $output = & powershell -NoProfile -ExecutionPolicy Bypass -File $setup -HermesHome $HomeDir -ConfigureOnly 2>&1 | Out-String
  return [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = $output }
}

# The leading comma stops PowerShell from unrolling the array on return: an empty result
# would otherwise arrive as $null and have no .Count under strict mode.
function Get-Backups([string]$HomeDir) {
  return , @(Get-ChildItem -LiteralPath $HomeDir -Filter '.env.bak-kairos-*' -Force)
}

function Get-KeyLines([string]$Text) {
  return , @([regex]::Matches($Text, '(?m)^API_SERVER_KEY=([0-9a-f]+)\r?$'))
}

function Assert([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

function Test-Case([string]$Name, [scriptblock]$Body) {
  $homeDir = New-FakeHome
  try {
    & $Body $homeDir
    $script:passed++
    Write-Host "  [pass] $Name" -ForegroundColor Green
  } catch {
    $script:failures++
    Write-Host "  [FAIL] $Name`n         $($_.Exception.Message)" -ForegroundColor Red
  } finally {
    Remove-Item -LiteralPath $homeDir -Recurse -Force
  }
}

Write-Host 'scripts/setup.ps1 tests'

Test-Case 'a missing .env is created with all three settings and a 64-hex key' {
  param($h)
  $r = Invoke-Setup $h
  Assert ($r.ExitCode -eq 0) "exit $($r.ExitCode): $($r.Output)"
  $text = [System.IO.File]::ReadAllText((Join-Path $h '.env'))
  Assert ($text -match '(?m)^API_SERVER_ENABLED=true\r?$') 'ENABLED not set'
  Assert ($text -match '(?m)^API_SERVER_HOST=127\.0\.0\.1\r?$') 'HOST not set'
  $keys = Get-KeyLines $text
  Assert ($keys.Count -eq 1 -and $keys[0].Groups[1].Value.Length -eq 64) 'expected one 64-hex key'
  Assert ((Get-Backups $h).Count -eq 0) 'no backup expected when there was no .env'
}

Test-Case 'a second run changes nothing and makes no backup' {
  param($h)
  $null = Invoke-Setup $h
  $envPath = Join-Path $h '.env'
  $before = [System.IO.File]::ReadAllBytes($envPath)
  $r = Invoke-Setup $h
  $after = [System.IO.File]::ReadAllBytes($envPath)
  Assert ($r.ExitCode -eq 0) "exit $($r.ExitCode)"
  Assert ($r.Output -match 'nothing changed') 'expected "nothing changed"'
  Assert ([Convert]::ToBase64String($before) -eq [Convert]::ToBase64String($after)) '.env bytes changed'
  Assert ((Get-Backups $h).Count -eq 0) 'unexpected backup'
}

Test-Case 'a weak key is replaced, other lines keep their exact bytes, and a backup is made' {
  param($h)
  $original = "# Hermes secrets`r`nOPENROUTER_API_KEY=sk-test-123`r`nAPI_SERVER_KEY=change-me-local-dev`r`nTERMINAL_TIMEOUT=`nLAST=1"
  Write-Bytes (Join-Path $h '.env') $original
  $r = Invoke-Setup $h
  Assert ($r.ExitCode -eq 0) "exit $($r.ExitCode): $($r.Output)"
  $text = [System.IO.File]::ReadAllText((Join-Path $h '.env'))
  Assert ($text.StartsWith("# Hermes secrets`r`nOPENROUTER_API_KEY=sk-test-123`r`nTERMINAL_TIMEOUT=`nLAST=1")) "other lines changed: [$text]"
  Assert ($text -notmatch 'change-me-local-dev') 'weak key still present'
  Assert ((Get-KeyLines $text).Count -eq 1) 'expected exactly one key line'
  $backups = Get-Backups $h
  Assert ($backups.Count -eq 1) 'expected one backup'
  Assert ([System.IO.File]::ReadAllText($backups[0].FullName) -eq $original) 'backup differs from original'
}

Test-Case 'a strong key is kept when only another setting is wrong' {
  param($h)
  $key = 'ab' * 32
  Write-Bytes (Join-Path $h '.env') "API_SERVER_ENABLED=true`nAPI_SERVER_HOST=0.0.0.0`nAPI_SERVER_KEY=$key`n"
  $r = Invoke-Setup $h
  Assert ($r.ExitCode -eq 0) "exit $($r.ExitCode)"
  $text = [System.IO.File]::ReadAllText((Join-Path $h '.env'))
  Assert ($text -match '(?m)^API_SERVER_HOST=127\.0\.0\.1$') 'HOST not fixed'
  Assert ($text -notmatch '0\.0\.0\.0') 'old HOST line still present'
  $keys = Get-KeyLines $text
  Assert ($keys.Count -eq 1 -and $keys[0].Groups[1].Value -eq $key) 'strong key was regenerated'
}

Test-Case 'quoted, exported and duplicate settings collapse to one line each' {
  param($h)
  Write-Bytes (Join-Path $h '.env') "export API_SERVER_ENABLED=`"false`"`nAPI_SERVER_ENABLED=false`n  API_SERVER_HOST = '127.0.0.1'`nAPI_SERVER_KEY=short`n"
  $r = Invoke-Setup $h
  Assert ($r.ExitCode -eq 0) "exit $($r.ExitCode)"
  $text = [System.IO.File]::ReadAllText((Join-Path $h '.env'))
  foreach ($name in 'API_SERVER_ENABLED', 'API_SERVER_HOST', 'API_SERVER_KEY') {
    $count = ([regex]::Matches($text, "(?m)^\s*(?:export\s+)?$name\s*=")).Count
    Assert ($count -eq 1) "$name appears $count times"
  }
  Assert ($text -match '(?m)^API_SERVER_ENABLED=true$') 'ENABLED not true'
}

Test-Case 'the key never appears in the output' {
  param($h)
  $r = Invoke-Setup $h
  $text = [System.IO.File]::ReadAllText((Join-Path $h '.env'))
  $key = (Get-KeyLines $text)[0].Groups[1].Value
  Assert (-not $r.Output.Contains($key)) 'key printed to output'
}

Test-Case 'the rewritten .env has no byte-order mark' {
  param($h)
  $null = Invoke-Setup $h
  $bytes = [System.IO.File]::ReadAllBytes((Join-Path $h '.env'))
  Assert (-not ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF)) 'BOM written'
}

Test-Case 'a home without config.yaml fails with exit code 1 and writes nothing' {
  param($h)
  Remove-Item -LiteralPath (Join-Path $h 'config.yaml')
  $r = Invoke-Setup $h
  Assert ($r.ExitCode -eq 1) "expected exit 1, got $($r.ExitCode)"
  Assert ($r.Output -match 'No Hermes config') 'missing error message'
  Assert (-not (Test-Path -LiteralPath (Join-Path $h '.env'))) '.env was created'
}

Write-Host "$script:passed passed, $script:failures failed"
if ($script:failures -gt 0) { exit 1 }
exit 0
