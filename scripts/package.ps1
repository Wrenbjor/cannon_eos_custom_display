#requires -Version 7.0
param([Parameter(Mandatory)][string]$Destination, [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    $destinationPath = [IO.Path]::GetFullPath($Destination)
    if ((Test-Path -LiteralPath $destinationPath) -or (Test-Path -LiteralPath "$destinationPath.zip")) {
        throw 'Choose a new package destination; existing files will not be replaced.'
    }
    if (git status --porcelain) { throw 'Commit the source before packaging so source.zip matches the release.' }
    # SkipBuild packages an already validated build (for example, when its GUI
    # remains open for a user's live test). It never bypasses source commit checks.
    if (-not $SkipBuild) {
        & (Join-Path $PSScriptRoot 'build.ps1')
    }
    $metadata = & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Dependency metadata failed' }
    $version = ($metadata.packages | Where-Object name -eq 'eos-camera' | Select-Object -First 1).version
    if (-not $version) { throw 'Cannot identify package version' }
    $commit = git rev-parse HEAD
    if ($LASTEXITCODE) { throw 'Cannot identify source revision' }
    New-Item -ItemType Directory -Path $destinationPath | Out-Null
    Copy-Item -LiteralPath 'target/release/open-eos-studio.exe','target/release/eos-camera.exe','LICENSE','README.md' -Destination $destinationPath
    Copy-Item -LiteralPath 'build/vcam/Release/OpenEosCameraSource.dll','build/vcam/Release/eos-vcam.exe' -Destination $destinationPath
    Copy-Item -LiteralPath 'scripts/register-test-camera.ps1' -Destination (Join-Path $destinationPath 'register-camera.ps1')
    Copy-Item -LiteralPath 'scripts/install-camera.ps1' -Destination (Join-Path $destinationPath 'Install virtual camera.ps1')
    [IO.File]::WriteAllText((Join-Path $destinationPath 'Install virtual camera.cmd'), '@echo off' + "`r`n" + 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install virtual camera.ps1"' + "`r`n" + 'pause' + "`r`n")
    Copy-Item -LiteralPath 'LICENSES' -Destination $destinationPath -Recurse
    Copy-Item -LiteralPath 'native/vcam/third_party/MICROSOFT-LICENSE.txt' -Destination (Join-Path $destinationPath 'LICENSES')
    Copy-Item -LiteralPath 'docs/hardware-validation.md' -Destination (Join-Path $destinationPath 'VALIDATION.md')
    Copy-Item -LiteralPath 'docs/building.md' -Destination (Join-Path $destinationPath 'BUILDING.md')
    & git archive --format=zip "--output=$destinationPath/source.zip" HEAD
    if ($LASTEXITCODE) { throw 'Source archive failed' }
    $notices = [Text.StringBuilder]::new()
    [void]$notices.AppendLine("Open EOS Studio - Rust dependency notices`nSource revision: $commit`n")
    $includedIds = @{}
    foreach ($node in $metadata.resolve.nodes) { $includedIds[$node.id] = $true }
    foreach ($package in ($metadata.packages | Sort-Object name,version)) {
        if (-not $package.source -or -not $includedIds.ContainsKey($package.id)) { continue }
        [void]$notices.AppendLine("`n========== $($package.name) $($package.version) ==========")
        [void]$notices.AppendLine("License: $($package.license)`nSource: $($package.repository)")
        $packageRoot = Split-Path $package.manifest_path -Parent
        $licenseFiles = @(Get-ChildItem -LiteralPath $packageRoot -File -Recurse | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|OFL)([.\-_]|$)' })
        if ($package.license_file) {
            $explicitLicense = Join-Path $packageRoot $package.license_file
            if (Test-Path -LiteralPath $explicitLicense) { $licenseFiles += Get-Item -LiteralPath $explicitLicense }
        }
        foreach ($file in ($licenseFiles | Sort-Object FullName -Unique)) {
            [void]$notices.AppendLine("`n--- $([IO.Path]::GetRelativePath($packageRoot, $file.FullName)) ---")
            [void]$notices.AppendLine([IO.File]::ReadAllText($file.FullName))
        }
    }
    [IO.File]::WriteAllText((Join-Path $destinationPath 'THIRD-PARTY-NOTICES.txt'), $notices.ToString())
    $startHere = @"
# Open EOS Studio $version

Requires Windows 11 x64 and the Microsoft Visual C++ x64 Redistributable:
https://aka.ms/vc14/vc_redist.x64.exe
Windows N editions also need Microsoft's Media Feature Pack. The app uses
Windows media components and a GPU driver supporting OpenGL 3.3 or newer.
This preview build is not code-signed. See README.md and BUILDING.md for
runtime prerequisites, compiler setup, source builds and troubleshooting.

Double-click open-eos-studio.exe. Connect and wake the Canon T3i / 600D.
Choose rotation/crop, select your separate microphone, then Start recording.
Stop and save finishes the MP4; Play last recording opens it in Windows.
Recordings are saved in the Recordings folder beside the app unless changed.

To use in OBS or another camera app:
1. Run Install virtual camera.cmd once and accept the Windows administrator prompt.
2. Open Studio, wait for live preview, then click Start virtual camera.
3. In OBS add a Video Capture Device and select Open EOS Camera (Windows Virtual Camera).
4. Set Resolution/FPS Type to Custom and use 704x1056 for the full portrait image,
   594x1056 for 9:16 portrait crop, 1056x704 for full landscape, or 1056x594 for 16:9.
5. Set the OBS canvas/output dimensions to match if you want that file shape.
6. Select your separate microphone in OBS. Keep Studio running.

OBS 32.0.2 live preview and recording are verified. Zoom, Meet and browser
compatibility still need individual tests. Rotation and crop come from Studio.
USB live view is 1056 x 704, not native 1080p or full-resolution stills.
Administrator access is needed only for the one-time virtual-camera registration.
No Canon Webcam Utility or replacement USB driver is needed.
Use one camera-consuming app at a time; mixed-resolution simultaneous
consumers are not working reliably yet.

Source revision: $commit
Repository: https://github.com/Wrenbjor/cannon_eos_custom_display
Corresponding project source is included in source.zip, with Cargo.lock and
build instructions. Rust dependencies can be retrieved using cargo fetch --locked.
See LICENSE, LICENSES, THIRD-PARTY-NOTICES.txt and VALIDATION.md.
"@
    [IO.File]::WriteAllText((Join-Path $destinationPath 'START-HERE.md'), $startHere)
    Compress-Archive -LiteralPath $destinationPath -DestinationPath "$destinationPath.zip"
    Write-Output "Packaged $destinationPath.zip (source $commit)"
} finally { Pop-Location }
