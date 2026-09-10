# Open EOS Camera

An open-source Windows utility for the Canon EOS 600D / Rebel T3i. The target is a standard selectable camera for Zoom, Google Meet, browsers, OBS, Windows Camera and other applications, with a companion interface for camera controls and original-photo capture.

**Development prototype — not yet a usable replacement webcam.** Two independent paths work: direct USB live-view acquisition and a Windows Media Foundation camera source with an animated test pattern. They are not connected yet. No Canon SDK, Webcam Utility subscription, firmware modification, or replacement USB driver is required by the implemented camera path.

## What works now

- Find the physical T3i using its existing Windows Portable Devices driver.
- Read advertised Canon commands and selected camera properties without printing owner names or USB serial numbers.
- Retrieve native live-view JPEG frames. The development camera returned **1056 × 704**.
- Rotate the actual pixels by 0°, 90°, 180° or 270°. A quarter turn produces **704 × 1056** output; it does not rely on EXIF/display rotation.
- Measure USB frame delivery with JPEG decoding and rotation included; Ctrl+C stops the benchmark and runs session cleanup.
- Experimental one-shot lens-drive and autofocus commands. Command acceptance is not proof of optical movement or successful focus lock.
- Build a native Windows camera source offering **1280 × 720** and **720 × 1280**, each in NV12 and RGB32. Automated tests activate the DLL and verify moving frames and timestamps in all four formats. These are synthetic output formats, not native sensor resolutions.

Still missing: USB frames in the virtual camera, desktop controls, full-resolution JPEG/CR2 capture/download, exposure controls, mode changes, local recording, installer, and real application compatibility testing. See [the development plan](docs/development-plan.md) and [validation record](docs/hardware-validation.md).

## Build on Windows 11 x64

Install Rust via rustup and Visual Studio 2022 Build Tools with **Desktop development with C++**, CMake, and a Windows 11 SDK (22621 or newer). The toolchain is pinned to Rust 1.94.1; Cargo.lock pins Rust dependencies. The native build downloads a SHA-256-pinned WIL package and uses the SDK's C++/WinRT headers.

```powershell
git clone git@github.com:Wrenbjor/cannon_eos_custom_display.git
cd cannon_eos_custom_display
./scripts/build.ps1
```

The build script compiles both components and runs formatting, lint, Rust unit tests and native streaming tests. Tests do not trigger the camera or require camera registration. The `.github/workflows/windows.yml` workflow runs these checks on a Windows runner.

## Use the camera diagnostic application

Connect the T3i by USB, switch it on and keep it awake. Close camera clients. Canon's `EWCService` and `EWCPairingService` may still be accessing the camera after its window closes; temporarily stop those services if connection or frame retrieval conflicts occur. This program does not stop services or change their startup settings.

```powershell
./target/release/eos-camera.exe list
./target/release/eos-camera.exe probe
./target/release/eos-camera.exe status
./target/release/eos-camera.exe preview --output captures/landscape.jpg
./target/release/eos-camera.exe preview --rotate 90 --output captures/portrait.jpg
./target/release/eos-camera.exe benchmark --frames 120 --rotate 90
```

Saved previews are live-view images, **not full-resolution still photos**. Non-rotated previews preserve the original USB JPEG bytes; rotated previews are decoded and re-encoded at JPEG quality 95. Output files are never overwritten. Images and build output are ignored by Git.

Experimental focus commands require a compatible electronically controlled lens, normally with its switch set to AF:

```powershell
./target/release/eos-camera.exe focus near --step 1
./target/release/eos-camera.exe focus far --step 1
./target/release/eos-camera.exe autofocus
```

Steps 1–3 are Canon's relative movement sizes, not distances. Autofocus runs for a bounded observation period and is then cancelled; it does not yet interpret focus-lock events. A focus command is never automatically repeated after an uncertain reply. The camera's physical mode dial remains authoritative.

The app serializes its own camera access within the Windows user session. It cannot coordinate competing Canon or third-party applications. Disconnects produce errors. Normal cleanup restores only live-view properties this process changed; if USB is removed or the process is killed, switch the camera off/on to reset it. Individual WPD calls can block according to the Windows driver's timeout, even though application retries are bounded.

## Test the Windows camera source

The build runs the DLL streaming test without registering a device. To make the **animated test pattern** selectable in camera applications, a development registration script is included. Its elevated registration path has not yet been validated on the development machine; inspect it before use.

From an Administrator PowerShell:

```powershell
./scripts/register-test-camera.ps1
```

The script copies the camera DLL and controller to a protected directory under Program Files and registers only this project's COM class. It does not replace the Canon camera's driver. Then, from a normal terminal:

```powershell
& "$env:ProgramFiles/Open EOS Camera Development/eos-vcam.exe" run
```

Select **Open EOS Camera (test pattern)** in a camera app. Press Enter in the controller to stop/remove the session device. The test source has no microphone. To remove the COM registration, stop the controller and run `./scripts/register-test-camera.ps1 -Unregister` from Administrator PowerShell. The script leaves installed files in place for deliberate removal after all clients exit.

**Zoom, Meet, browser, OBS and Windows Camera compatibility is unverified.** Passing a Media Foundation source-reader test does not establish that those clients can select or use the registered device. The next milestone must validate them individually.

## Next development slice

1. Connect the Rust camera worker to the native camera source through versioned local IPC, with bounded frame storage, timestamps, explicit access controls and disconnect handling.
2. Add a desktop preview with clockwise/counterclockwise rotation, fit/crop, and persistent portrait/landscape profiles.
3. Register and test the real feed in the required application matrix; add a DirectShow adapter if actual client tests require it.
4. Add original-photo capture/download and verified exposure/focus controls. Preserve originals independently of preview transforms.

## License and provenance

Project code is GPL-3.0-or-later. The Windows source adapter is adapted from Microsoft's MIT-licensed Windows-Camera sample; its original notices are retained. Canon protocol layouts and sequencing were developed using libgphoto2's LGPL-2.1-or-later implementation as a reference. See [protocol and dependency sources](docs/protocol-sources.md), LICENSE, LICENSES and the native adapter's third_party directory. This project is independent of Canon.
