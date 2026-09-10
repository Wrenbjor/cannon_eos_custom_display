param([Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    $destinationPath = [IO.Path]::GetFullPath($Destination)
    if ((Test-Path -LiteralPath $destinationPath) -or (Test-Path -LiteralPath "$destinationPath.zip")) {
        throw 'Choose a new package destination; existing files will not be replaced.'
    }
    if (git status --porcelain) { throw 'Commit the source before packaging so source.zip matches the release.' }
    & cargo build --release --locked
    if ($LASTEXITCODE) { throw 'Release build failed' }
    $metadata = & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Dependency metadata failed' }
    $commit = git rev-parse HEAD
    if ($LASTEXITCODE) { throw 'Cannot identify source revision' }
    New-Item -ItemType Directory -Path $destinationPath | Out-Null
    Copy-Item -LiteralPath 'target/release/open-eos-studio.exe','target/release/eos-camera.exe','LICENSE','README.md' -Destination $destinationPath
    Copy-Item -LiteralPath 'LICENSES' -Destination $destinationPath -Recurse
    Copy-Item -LiteralPath 'native/vcam/third_party/MICROSOFT-LICENSE.txt' -Destination (Join-Path $destinationPath 'LICENSES')
    Copy-Item -LiteralPath 'docs/hardware-validation.md' -Destination (Join-Path $destinationPath 'VALIDATION.md')
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
# Open EOS Studio 0.2.0

Double-click open-eos-studio.exe. Connect and wake the Canon T3i / 600D.
Choose rotation/crop, select your separate microphone, then Start recording.
Stop and save finishes the MP4; Play last recording opens it in Windows.
Recordings are saved in the Recordings folder beside the app unless changed.

This is a desktop recorder. Sending the real camera to Zoom, Meet, browsers
and OBS through the system-wide virtual camera remains the next milestone.
USB live view is 1056 x 704, not native 1080p or full-resolution stills.
No administrator access or Canon Webcam Utility is needed for recording.

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
