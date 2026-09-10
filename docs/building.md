# Build Open EOS Studio on Windows

The supported development target is **Windows 11 x64 / x86_64-pc-windows-msvc**. The desktop app and USB worker are Rust; the Windows virtual-camera source is C++20. This repository does not currently provide Linux, macOS, Windows 10, ARM64 or 32-bit builds. Run these commands in Windows, not WSL or a MinGW shell.

## If you only want to use the app

Download the Windows x64 ZIP from [GitHub Releases](https://github.com/Wrenbjor/cannon_eos_custom_display/releases), extract the entire folder, and follow START-HERE.md. You do not need Rust, Visual Studio, CMake, Git, PowerShell 7, Python, Node.js or FFmpeg to run a release.

Runtime prerequisites:

- **Windows 11 x64.** Development hardware testing used Windows 11 Home build 26200. The virtual-camera API requires Windows 11; not every Windows edition/build has been tested.
- **Microsoft Visual C++ x64 Redistributable**, current v14 release: [official x64 installer](https://aka.ms/vc14/vc_redist.x64.exe), [Microsoft's documentation](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist). The binaries import VCRUNTIME140.dll; the native camera source also imports MSVCP140.dll and VCRUNTIME140_1.dll. Installing the runtime is sufficient; users do not need the compiler.
- **Windows media components.** Windows N editions require Microsoft's [Media Feature Pack](https://support.microsoft.com/en-us/windows/experience/platform-variants/media-feature-pack-for-windows-n). H.264/AAC encoding uses Media Foundation.
- **A working graphics driver with OpenGL 3.3 or newer** for the desktop preview. Use the GPU manufacturer's driver if the window cannot initialize graphics; remote desktop/VM graphics have not been validated.
- **Canon EOS 600D / Rebel T3i and a USB data cable.** Keep the camera powered and awake. Windows must see the physical camera through its existing Portable Devices driver. A separate microphone is optional.
- **Administrator rights once** to register the virtual-camera DLL. Normal preview, recording and camera sharing run as the signed-in user. Windows' built-in PowerShell runs the installer. Close camera clients before installing or updating.

The release is a preview and is not code-signed. Obtain it from this repository's Releases page and compare its SHA-256 if needed. Do not disable Windows security protections or download individual runtime DLLs from third-party sites. Canon's Webcam Utility, EDSDK and camera firmware modifications are not dependencies.

## Install the development prerequisites

Install these before cloning/building. Reopen your terminal after installations so PATH changes take effect.

| Tool | What to download / select | Used for |
|---|---|---|
| Git for Windows | [Official x64 installer](https://git-scm.com/install/windows); allow Git on PATH | Clone source, identify revisions and build source archives |
| PowerShell 7 | [Microsoft installation guide](https://learn.microsoft.com/en-us/powershell/scripting/install/install-powershell-on-windows); command is `pwsh` | Build and packaging scripts; Windows PowerShell 5.1 is insufficient for packaging |
| Visual Studio 2022 Build Tools or newer | [Microsoft downloads](https://visualstudio.microsoft.com/downloads/) → Build Tools; select **Desktop development with C++** (called **C++ build tools** in some installers) | Native compiler, linker and C++ standard library |
| MSVC x64/x86 tools | In that workload, keep the x64/x86 MSVC toolset selected (v143 for VS 2022 or the newer toolset matching your VS version) | Rust's MSVC linker and the C++20 DLL |
| Windows 11 SDK | Select **10.0.22621.0 or newer**; local development used 10.0.26100.0 | WPD, Media Foundation, virtual-camera and C++/WinRT headers/libraries |
| C++ CMake tools for Windows | Select this component in Visual Studio Installer; CMake must be **3.24 or newer** | Configure/build the native source. The build script finds the VS-bundled CMake when it is not on PATH |
| Rust via rustup | [Official Windows installer](https://rust-lang.org/tools/install/); choose the **x86_64-pc-windows-msvc** host, not GNU | Rust compiler, Cargo, rustfmt and Clippy |

The full Visual Studio IDE is optional; Build Tools contains what the build needs. Visual Studio Code alone does not include a C++ compiler or SDK.

The repository pins Rust **1.94.1** in `rust-toolchain.toml`. Rustup installs that toolchain and its `rustfmt`/`clippy` components when first invoked here. To install them explicitly:

```powershell
rustup toolchain install 1.94.1 --profile minimal --component rustfmt --component clippy
```

The first build needs internet access for the Rust toolchain/crates and NuGet. Cargo.lock pins crate versions; CMake downloads WIL **1.0.260126.7** and verifies its pinned SHA-256. WIL is automatic: no separate Canon SDK, libgphoto2 installation, NuGet CLI or C++/WinRT package is required. Allow several gigabytes for compiler installations, dependency caches and build output; exact size varies by installed workloads.

## Clone and compile

Open **PowerShell 7** (`pwsh`) as a normal user. HTTPS cloning does not require a GitHub account or SSH key:

```powershell
git clone https://github.com/Wrenbjor/cannon_eos_custom_display.git
cd cannon_eos_custom_display
git --version
cargo --version
./scripts/build.ps1
```

The script builds optimized Rust binaries and the native C++ source, then runs rustfmt, Clippy with warnings denied, Rust unit/encoder tests, twelve native streaming formats and a Rust-to-native pipe test. A successful final line lists the generated binaries. Tests use generated media, require no camera or microphone, and do not register a camera device.

If your execution policy blocks a reviewed checkout's scripts, run this process-scoped command rather than changing the machine-wide policy:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/build.ps1
```

For a specific release, check out its tag before building, for example `git switch --detach v0.3.0`. Keep the checkout path reasonably short and writable. Avoid building in Program Files. Close any Studio executable launched from `target/release` before rebuilding it, since Windows locks running executables.

| Output | Purpose |
|---|---|
| `target/release/open-eos-studio.exe` | Desktop preview, local recording and camera sharing |
| `target/release/eos-camera.exe` | Camera diagnostics and recording CLI |
| `build/vcam/Release/OpenEosCameraSource.dll` | Windows camera source loaded by the media pipeline |
| `build/vcam/Release/eos-vcam.exe` | Native source diagnostic/self-test controller |

Launch the GUI with `./target/release/open-eos-studio.exe`. To share with OBS, run `./scripts/install-camera.ps1` once, accept Windows elevation, then click **Start virtual camera** in Studio. Building and running the tests do not install or replace any driver.

## Smaller development commands

After the prerequisites are installed:

```powershell
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo build --release --locked
```

The final command builds only Rust executables. It does not build/install the C++ camera source; use the full build script for a complete application. `./scripts/build.ps1 -SkipTests` is available for local iteration; do not use it to validate a release. Debug builds are substantially slower and should not be used for recording-performance measurements.

## Package a build

Commit all source changes first. Packaging refuses a dirty checkout or an existing destination, so `source.zip` corresponds to a definite Git revision.

```powershell
./scripts/package.ps1 -Destination ../Open-EOS-Studio-local
```

This builds and tests all components, then writes the folder and `../Open-EOS-Studio-local.zip`. The ZIP contains the GUI, CLI, source DLL/controller, registration scripts, source archive, build instructions and third-party license notices. Test recordings and developer machine settings are excluded. `-SkipBuild` is only for packaging binaries already built and tested from the intended source revision.

The `.github/workflows/release.yml` workflow performs a fresh Windows build and all checks when a maintainer pushes a matching version tag. It publishes a **preview release**, the Windows x64 ZIP, and a `.zip.sha256` checksum file. The tag must exactly match the Cargo package version. Release notes live in `docs/release-notes.md`. Do not move a published tag to change its binaries; increment the version instead.

## Troubleshooting compilation and startup

| Symptom | Fix |
|---|---|
| `cargo`, `git` or `pwsh` is not recognized | Reopen the terminal; verify the corresponding installer added its command to PATH |
| `link.exe` missing, MSVC not found, or CMake cannot find Visual Studio | Modify Visual Studio installation and add the C++ workload, x64/x86 MSVC tools and Windows 11 SDK; retry from VS's **Developer PowerShell** or **x64 Native Tools Command Prompt**, then launch `pwsh` |
| `mfvirtualcamera.h`, `windows.h` or C++/WinRT headers missing | Install/select Windows 11 SDK 22621 or newer. Changing SDK/VS versions in an existing CMake cache may require a fresh `build/vcam` build directory |
| CMake is missing | Add **C++ CMake tools for Windows** in Visual Studio Installer; the script locates the bundled executable with vswhere |
| Script requires PowerShell 7 / GetRelativePath is unavailable | Use `pwsh`, not `powershell.exe`, for build/packaging; the camera installer intentionally uses built-in Windows PowerShell |
| Access denied replacing an EXE or source DLL | Close Studio and camera clients before rebuilding/updating; the registered DLL can remain loaded in Windows Frame Server until consumers exit |
| VCRUNTIME140.dll / MSVCP140.dll missing on another PC | Install Microsoft's current **x64** Visual C++ Redistributable, linked above |
| Missing Media Foundation / H.264 / AAC support | On Windows N, install the Media Feature Pack and restart; do not download standalone codec DLLs |
| CMake/NuGet or Cargo download fails | Check internet/proxy access and retry. Do not remove the WIL hash check or rewrite Cargo.lock just to bypass a download failure |
| No physical T3i detected | Confirm USB data connection and camera power; stop competing Canon background camera services. Seeing Canon's virtual webcam alone does not mean Windows sees the physical DSLR |

When reporting a build problem, include Windows edition/build, architecture, Visual Studio/SDK versions and the first actual compiler error. Remove camera serials, owner fields and personal paths/media from public logs.
