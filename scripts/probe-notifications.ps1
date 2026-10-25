<#
.SYNOPSIS
  Can an unpackaged desktop app read Windows notifications? (M4 Step 1)

.DESCRIPTION
  Kairos is an ordinary desktop app with no package identity, and Microsoft documents
  UserNotificationListener for packaged apps. This probe asks the same question from Windows
  PowerShell, which is also unpackaged:

  1. Reads the listener's access status, and asks for access if Windows hasn't decided yet.
  2. Shows a test toast ("Kairos probe").
  3. Reads the notifications back and looks for that toast.

  It prints counts and app names only. Message text is printed only with -ShowText, so the output
  is safe to paste into a conversation. It changes nothing on the PC except showing one toast.

  Run it in Windows PowerShell 5.1 (powershell.exe), not PowerShell 7: the WinRT types it uses are
  only projected in 5.1.

  Verdict, also the exit code:
    0 Readable     the test toast was read back: M4 Step 4 can go ahead
    1 Denied       Windows refused access (see the hint it prints)
    2 Unsupported  the listener isn't available to this process
    3 Inconclusive access allowed but the test toast wasn't found (rerun once; then report)
#>
[CmdletBinding()]
param(
  [switch]$ShowText
)

Set-StrictMode -Version 3.0
$ErrorActionPreference = 'Stop'

function Write-Ok([string]$Message) { Write-Host "  [ok] $Message" -ForegroundColor Green }
function Write-Step([string]$Message) { Write-Host "  $Message" }
function Finish([int]$Code, [string]$Verdict, [string]$Detail) {
  $color = if ($Code -eq 0) { 'Green' } else { 'Yellow' }
  Write-Host ''
  Write-Host "  Verdict: $Verdict" -ForegroundColor $color
  if ($Detail) { Write-Host "  $Detail" }
  exit $Code
}

if ($PSVersionTable.PSVersion.Major -ne 5) {
  Finish 2 'Unsupported' "Run this in Windows PowerShell 5.1 (powershell.exe); this is $($PSVersionTable.PSVersion)."
}

Write-Host 'Kairos notification probe (M4 Step 1)'

# Await a WinRT IAsyncOperation<T> from PowerShell 5.1.
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTaskGeneric = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
  $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and
  $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
} | Select-Object -First 1
function Wait-WinRt($Operation, [Type]$ResultType) {
  $task = $asTaskGeneric.MakeGenericMethod($ResultType).Invoke($null, @($Operation))
  if (-not $task.Wait(15000)) { throw 'Windows did not answer within 15 s' }
  return $task.Result
}

try {
  [void][Windows.UI.Notifications.Management.UserNotificationListener, Windows.UI.Notifications, ContentType = WindowsRuntime]
  [void][Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]
  [void][Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime]
  $listener = [Windows.UI.Notifications.Management.UserNotificationListener]::Current
} catch {
  Finish 2 'Unsupported' "The listener isn't available here: $($_.Exception.Message)"
}

# --- 1. Access -----------------------------------------------------------------------------------

$statusType = [Windows.UI.Notifications.Management.UserNotificationListenerAccessStatus]
try {
  $status = $listener.GetAccessStatus()
} catch {
  Finish 2 'Unsupported' "GetAccessStatus failed: $($_.Exception.Message)"
}
Write-Step "Access status: $status"

if ($status -eq 'Unspecified') {
  Write-Step 'Asking Windows for access (a prompt may appear)...'
  try {
    $status = Wait-WinRt ($listener.RequestAccessAsync()) $statusType
    Write-Step "Access status after asking: $status"
  } catch {
    Write-Step "RequestAccessAsync failed: $($_.Exception.Message)"
  }
}

if ($status -ne 'Allowed') {
  Finish 1 'Denied' ("Windows says '$status'. Check Settings > Privacy & security > Notifications " +
    '("Notification access" / "Let apps access your notifications") is on, then run this again. ' +
    'If it stays Denied with that setting on, unpackaged apps are refused on this PC.')
}
Write-Ok 'Access allowed'

# --- 2. Test toast -------------------------------------------------------------------------------

$marker = "Kairos probe $(Get-Random -Minimum 1000 -Maximum 9999)"
# Windows PowerShell's own AppUserModelID, so the toast needs no registered app.
$appId = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'
try {
  $template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02
  $xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template)
  $texts = $xml.GetElementsByTagName('text')
  [void]$texts.Item(0).AppendChild($xml.CreateTextNode($marker))
  [void]$texts.Item(1).AppendChild($xml.CreateTextNode('Test toast for the M4 notification probe.'))
  $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
  [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
  Write-Ok "Showed a test toast: '$marker'"
} catch {
  Write-Step "Couldn't show a test toast ($($_.Exception.Message)); reading existing notifications only."
  $marker = $null
}
Start-Sleep -Seconds 3

# --- 3. Read back --------------------------------------------------------------------------------

$kinds = [Windows.UI.Notifications.NotificationKinds]::Toast
$listType = [System.Collections.Generic.IReadOnlyList[Windows.UI.Notifications.UserNotification]]
try {
  $notifications = Wait-WinRt ($listener.GetNotificationsAsync($kinds)) $listType
} catch {
  Finish 2 'Unsupported' "Access is allowed but reading failed: $($_.Exception.Message)"
}

$generic = [Windows.UI.Notifications.KnownNotificationBindings]::ToastGeneric
$found = $false
$byApp = @{}
foreach ($n in $notifications) {
  $app = try { $n.AppInfo.DisplayInfo.DisplayName } catch { '(unknown app)' }
  $byApp[$app] = 1 + $(if ($byApp.ContainsKey($app)) { $byApp[$app] } else { 0 })
  $lines = @()
  try {
    $binding = $n.Notification.Visual.GetBinding($generic)
    if ($binding) { $lines = @($binding.GetTextElements() | ForEach-Object { $_.Text }) }
  } catch { }
  if ($marker -and ($lines -contains $marker)) { $found = $true }
  if ($ShowText) { Write-Step ("  [{0}] {1}: {2}" -f $n.CreationTime.ToLocalTime().ToString('HH:mm'), $app, ($lines -join ' | ')) }
}

Write-Ok "Read $(@($notifications).Count) notification(s) from Action Center"
foreach ($app in ($byApp.Keys | Sort-Object)) { Write-Step "  $app : $($byApp[$app])" }

if ($found) { Finish 0 'Readable' 'The test toast was read back: an unpackaged app can read notifications on this PC.' }
if (-not $marker) { Finish 3 'Inconclusive' 'No test toast could be shown; the counts above show what was readable.' }
Finish 3 'Inconclusive' ('Access is allowed, but the test toast was not in the list. Check that ' +
  'notifications are on for Windows PowerShell and Focus / Do not disturb is off, then run again.')
