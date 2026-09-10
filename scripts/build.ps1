param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    $cmakeCommand = Get-Command cmake -ErrorAction SilentlyContinue
    if ($cmakeCommand) { $cmakePath = $cmakeCommand.Source }
    else {
        $vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
        $vsPath = & $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (-not $vsPath) { throw 'Install Visual Studio 2022 Build Tools with Desktop development with C++ and a Windows 11 SDK.' }
        $cmakePath = Join-Path $vsPath 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe'
    }
    & cargo build --release --locked
    if ($LASTEXITCODE) { throw 'Rust build failed' }
    & $cmakePath -S native/vcam -B build/vcam -A x64
    if ($LASTEXITCODE) { throw 'CMake configuration failed' }
    & $cmakePath --build build/vcam --config Release --parallel
    if ($LASTEXITCODE) { throw 'Native camera build failed' }
    if (-not $SkipTests) {
        & cargo fmt --all -- --check
        if ($LASTEXITCODE) { throw 'Rust formatting check failed' }
        & cargo clippy --all-targets --locked -- -D warnings
        if ($LASTEXITCODE) { throw 'Rust lint checks failed' }
        & cargo test --locked
        if ($LASTEXITCODE) { throw 'Rust tests failed' }
        $ctestPath = Join-Path (Split-Path $cmakePath) 'ctest.exe'
        & $ctestPath --test-dir build/vcam -C Release --output-on-failure
        if ($LASTEXITCODE) { throw 'Virtual-camera streaming test failed' }
        & ./target/release/eos-camera.exe bridge-test --reader ./build/vcam/Release/eos-vcam.exe --source ./build/vcam/Release/OpenEosCameraSource.dll
        if ($LASTEXITCODE) { throw 'Cross-process camera bridge test failed' }
    }
    Write-Host 'Built target/release/{open-eos-studio.exe,eos-camera.exe} and build/vcam/Release/{eos-vcam.exe,OpenEosCameraSource.dll}'
} finally { Pop-Location }
