param([switch]$Unregister)
$ErrorActionPreference = 'Stop'
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $argsList = '-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '"'
    if ($Unregister) { $argsList += ' -Unregister' }
    $installer = Start-Process -FilePath powershell.exe -Verb RunAs -WindowStyle Hidden -ArgumentList $argsList -Wait -PassThru
    if ($installer.ExitCode) { throw 'Camera registration failed. See camera-install.log beside this script.' }
    exit
}
try {
    $packaged = Test-Path -LiteralPath (Join-Path $PSScriptRoot 'OpenEosCameraSource.dll')
    if ($packaged) {
        & (Join-Path $PSScriptRoot 'register-camera.ps1') -BinaryDirectory $PSScriptRoot -Unregister:$Unregister
    } else {
        & (Join-Path $PSScriptRoot 'register-test-camera.ps1') -Unregister:$Unregister
    }
    'Success. Open Studio and click Start virtual camera.' | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'camera-install.log')
} catch {
    $_.Exception.Message | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'camera-install.log')
    exit 1
}
