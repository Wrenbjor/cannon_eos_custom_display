# Development-only registration. Run in an Administrator PowerShell after building.
param([switch]$Unregister)
$ErrorActionPreference = 'Stop'
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'This operation needs Administrator PowerShell to register a Windows camera source. Building and self-tests do not need elevation.'
}
$classKey = 'HKLM:\SOFTWARE\Classes\CLSID\{EAD49F23-8F8C-47E7-A78C-F7744A6D54C9}'
$installDir = Join-Path $env:ProgramFiles 'Open EOS Camera Development'
$installedDll = Join-Path $installDir 'OpenEosCameraSource.dll'
if ($Unregister) {
    if (Test-Path -LiteralPath $classKey) {
        $registeredDll = (Get-Item -LiteralPath "$classKey\InprocServer32").GetValue('')
        if ($registeredDll -ne $installedDll) { throw 'The class points to another installation; refusing to remove it.' }
        Remove-Item -LiteralPath $classKey -Recurse
    }
    Write-Host "COM registration removed. Stop eos-vcam before removing the development files from $installDir."
    exit
}
$repoRoot = Split-Path $PSScriptRoot -Parent
$binaryDir = Join-Path $repoRoot 'build\vcam\Release'
foreach ($file in @('OpenEosCameraSource.dll', 'eos-vcam.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $binaryDir $file))) { throw 'Build the native camera first using scripts/build.ps1.' }
}
# The Frame Server loads this DLL. Keep the registered copy in an administrator-
# protected location, never a normal user's writable checkout or Downloads folder.
if (Test-Path -LiteralPath $installDir) {
    if ((Get-Item -LiteralPath $installDir).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Installation directory must not be a junction or symbolic link.' }
}
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
$secureAcl = New-Object Security.AccessControl.DirectorySecurity
$secureAcl.SetAccessRuleProtection($true, $false)
$inherit = [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'
$propagation = [Security.AccessControl.PropagationFlags]::None
foreach ($sidText in @('S-1-5-18', 'S-1-5-32-544')) {
    $sid = New-Object Security.Principal.SecurityIdentifier($sidText)
    $rule = New-Object Security.AccessControl.FileSystemAccessRule($sid, 'FullControl', $inherit, $propagation, 'Allow')
    $secureAcl.AddAccessRule($rule)
}
$users = New-Object Security.Principal.SecurityIdentifier('S-1-5-32-545')
$secureAcl.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($users, 'ReadAndExecute', $inherit, $propagation, 'Allow')))
Set-Acl -LiteralPath $installDir -AclObject $secureAcl
Copy-Item -LiteralPath (Join-Path $binaryDir 'OpenEosCameraSource.dll'), (Join-Path $binaryDir 'eos-vcam.exe') -Destination $installDir -Force
New-Item -Path "$classKey\InprocServer32" -Force | Out-Null
Set-Item -LiteralPath $classKey -Value 'Open EOS Camera development source'
Set-Item -LiteralPath "$classKey\InprocServer32" -Value $installedDll
New-ItemProperty -LiteralPath "$classKey\InprocServer32" -Name ThreadingModel -Value Both -PropertyType String -Force | Out-Null
Write-Host "Registered the animated test source. In a normal terminal, run: & '$installDir\eos-vcam.exe' run"
Write-Host 'This development source displays a TEST PATTERN; the USB camera feed is not connected to it yet.'
