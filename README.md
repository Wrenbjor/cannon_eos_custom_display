# Open EOS Studio / Open EOS Camera

An open-source Windows utility for the Canon EOS 600D / Rebel T3i. The target is a standard selectable camera for Zoom, Google Meet, browsers, OBS, Windows Camera and other applications, with a companion interface for camera controls and original-photo capture.

**Version 0.3 connects the real USB camera to a Windows virtual camera. OBS 32.0.2 preview and recording have been verified.** Studio also provides local MP4 recording with a separate microphone. Zoom, Meet and browser compatibility still need individual tests. No Canon SDK, Webcam Utility subscription, firmware modification, or replacement USB driver is required.

## Download a compiled build

Get the **Windows x64 ZIP** from [the v0.3.1 preview release](https://github.com/Wrenbjor/cannon_eos_custom_display/releases/tag/v0.3.1), extract the entire folder, and read START-HERE.md. Choose the asset named `Open-EOS-Studio-0.3.1-windows-x64.zip`; GitHub's "Source code" archives do not contain executables. A SHA-256 checksum accompanies the ZIP.

To run it, you need **Windows 11 x64**, Microsoft's current [Visual C++ x64 Redistributable](https://aka.ms/vc14/vc_redist.x64.exe), Windows media components and a graphics driver supporting OpenGL 3.3 or newer. Windows N editions also require the [Media Feature Pack](https://support.microsoft.com/en-us/windows/experience/platform-variants/media-feature-pack-for-windows-n). No Rust, C++ compiler, CMake, Python, FFmpeg or PowerShell 7 installation is needed to run the compiled app. Administrator rights are needed once for virtual-camera registration; normal use runs without elevation. The preview binaries are not code-signed.

Want to compile it? See **[Build on Windows 11 x64](#build-on-windows-11-x64)** below and the **[complete setup and troubleshooting guide](docs/building.md)**.

## Use the camera in OBS

1. From a packaged build, run **Install virtual camera.cmd** once and accept the Windows administrator prompt. From source, run `./scripts/install-camera.ps1`. This registers the source DLL in a protected Program Files directory.
2. Open **open-eos-studio.exe**, wait for live preview, set rotation/crop and click **Start virtual camera**. Keep Studio running. The `--virtual-camera` launch option starts sharing when preview becomes ready.
3. In OBS, add a **Video Capture Device** source and choose **Open EOS Camera (Windows Virtual Camera)**.
4. Set **Resolution/FPS Type → Custom**. Match Studio's default **1920 × 1080** landscape output, or choose **1080 × 1920** for Portrait / mobile. Use 30 fps for Windows output; this repeats the latest available USB frame, not 30 unique sensor frames per second. With **Native USB** quality, match full portrait **704 × 1056**, portrait crop **594 × 1056**, full landscape **1056 × 704**, or landscape crop **1056 × 594**.
5. Under OBS Settings → Video, match the base canvas and output dimensions to the file shape you want. Select your separate microphone in OBS or add an Audio Input Capture source.

Studio rotates the pixels before sharing, so no OBS source rotation is required. If OBS requests a different shape, the source fits the image with black borders rather than stretching it. 1280 × 720 and 720 × 1280 compatibility formats are also offered; they do not add camera detail. Matching native dimensions avoids resizing.

**Upgrading from 0.3.0:** close Studio and camera clients, then run the new virtual-camera installer to update the DLL's available resolutions. Studio now starts at **0° landscape**, replacing the old 90° default. Reset any compensating 270° rotation in OBS/TikTok to **0°**. Choose Landscape, Portrait / mobile, or Rotate 90° in Studio. Apps may need their capture source reopened to select a new resolution.

Uncheck **Show Studio preview** to stop its image uploads and reduce redraws while capture, local recording and virtual-camera output continue. Minimizing Studio also pauses image uploads. Keep Studio running; closing it stops capture. The **Native USB** quality option also avoids Studio's 1080 scaling work.

Stop virtual camera removes the session device; closing Studio also stops it. On USB disconnect the last frame expires after one second, then the device is removed as camera cleanup completes. Reconnect the camera and start sharing again. Do not reinstall while camera clients use the DLL. To remove COM registration, close clients and run `./scripts/install-camera.ps1 -Unregister` (packaged script: `Install virtual camera.ps1`). Installed files remain for later removal.

**Current limit:** a second DirectShow client requesting a different resolution while OBS used the camera returned an I/O error. Use one consuming application for now. Multiple pipe readers do not establish Windows client interoperability.

## Record a video

Launch **open-eos-studio.exe** from an extracted build, or `target/release/open-eos-studio.exe` after building. This is a normal desktop application; it does not need administrator rights or camera registration.

1. Connect the T3i by USB and switch it on. Stop conflicting camera clients. If it was asleep or disconnected, use **Reconnect camera**.
2. Choose **Landscape** or **Portrait / mobile** while watching the preview. Landscape starts at 0° and 1920 × 1080; Portrait / mobile applies a 90° clockwise turn and outputs 1080 × 1920. Use Rotate 90° or Fine rotation if the camera is mounted the other way. Crops take the center without stretching; **1080 output** upscales those pixels. Choose **Native USB** to disable that scaling. Full camera image preserves the available field of view.
3. Select your separate microphone, Windows default, or No audio. Microphones must currently use a 44.1 or 48 kHz mono/stereo Windows format. Plug in microphones before launching the app.
4. Choose the save folder and click **Start recording**. The microphone meter runs during recording. Click **Stop and save**, then **Play last recording**.

Recordings default to a `Recordings` folder beside the executable. Framing and microphone choices are locked during a recording. Closing the window finishes an active recording before exiting; a camera disconnect also attempts to finish the file. Do not force-kill the app while it is saving. Windows driver calls can delay cleanup.

MP4 files contain H.264 video and optional AAC audio, using Windows encoders. Preview and saved video use the same transformed pixels. Actual frame timestamps preserve elapsed time when USB delivery varies. Files are never overwritten. An interrupted or failed recording may remain as `*.recording.mp4`; that name means successful completion was not confirmed. Settings are not yet persisted between launches.

**Resolution:** this T3i supplies 1056 × 704 USB preview pixels, or 704 × 1056 after a quarter turn. A rotated 9:16 center crop is 594 × 1056; a landscape 16:9 crop is 1056 × 594. The default 1080 output scales those crops to 1080 × 1920 or 1920 × 1080. Full camera image with 1080 quality keeps the 3:2 aspect ratio at 1620 × 1080 (or 1080 × 1620); the virtual-camera canvas fits that image with borders. Upscaling adds no native detail. These remain live-view recordings, not native 1080p sensor video or full-resolution still photos.

## What works now

- Find the physical T3i using its existing Windows Portable Devices driver.
- Read advertised Canon commands and selected camera properties without printing owner names or USB serial numbers.
- Retrieve native live-view JPEG frames. The development camera returned **1056 × 704**.
- Rotate the actual pixels by 0°, 90°, 180° or 270°. A quarter turn produces **704 × 1056** output; it does not rely on EXIF/display rotation.
- Preview, center crop and record MP4 video in a desktop window, with a separate Windows microphone and a recording level meter.
- Measure USB frame delivery with JPEG decoding and rotation included; Ctrl+C stops the benchmark and runs session cleanup.
- Near/Far lens drive with Small, Medium and Large steps; Studio defaults to Large. The user confirmed physical movement with step 3 on the development camera. Autofocus continues retrieving frames during its bounded request; no focus-lock claim is made.
- Experimental preview-only focus markers from valid, recent Canon FocusInfoEx data. Selected points are yellow; this is not a focus-lock indication. The development T3i currently reports no point positions in Live AF mode, so marker alignment remains unverified. No invented target box is drawn; output video stays free of overlays.
- Share the transformed live feed as a Windows camera. Eight resolutions are offered in NV12 and RGB32; automated tests exercise all sixteen formats and cross-process RGB transport and offline clearing, including both 1080 shapes.

Still missing: full-resolution JPEG/CR2 capture/download, exposure controls, mode changes, persistent profiles, a signed production installer, reliable multi-application use, and remaining application compatibility tests. See [the development plan](docs/development-plan.md) and [validation record](docs/hardware-validation.md).

## Build on Windows 11 x64

Install the following on **Windows 11 x64**. Build in Windows PowerShell 7, not WSL or a MinGW shell.

| Prerequisite | Download / installer selection |
|---|---|
| Git for Windows | [Official x64 installer](https://git-scm.com/install/windows); add Git to PATH |
| PowerShell 7 | [Microsoft installation guide](https://learn.microsoft.com/en-us/powershell/scripting/install/install-powershell-on-windows); use `pwsh` |
| Visual Studio 2022 Build Tools or newer | [Microsoft downloads](https://visualstudio.microsoft.com/downloads/); select **Desktop development with C++** / **C++ build tools** |
| MSVC x64/x86 compiler and linker | Keep the C++ workload's x64/x86 toolset selected; C++20 is required |
| Windows 11 SDK | Select **10.0.22621.0 or newer**; local development used 10.0.26100.0 |
| CMake 3.24 or newer | Select **C++ CMake tools for Windows** in Visual Studio Installer; the script can find its bundled CMake |
| Rust via rustup | [Official installer](https://rust-lang.org/tools/install/); use the **x86_64-pc-windows-msvc** host toolchain |

Reopen the terminal after installation. The full Visual Studio IDE and Visual Studio Code are optional. Rustup reads the pinned **Rust 1.94.1**, rustfmt and Clippy requirements from `rust-toolchain.toml`. Cargo.lock pins crates; the native build downloads a SHA-256-pinned WIL package automatically. The first build needs internet access and several gigabytes of disk space for tools/caches. No Canon SDK or libgphoto2 installation is needed.

```powershell
git clone https://github.com/Wrenbjor/cannon_eos_custom_display.git
cd cannon_eos_custom_display
./scripts/build.ps1
```

Run those commands in **PowerShell 7** as a normal user. If execution policy blocks the reviewed script, use `pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/build.ps1`. HTTPS cloning does not require an SSH key or GitHub login.

The full script compiles the GUI, CLI and C++ source, then runs formatting, lint, Rust tests, sixteen native streaming formats and a cross-process camera bridge test. Tests use generated frames/audio and temporary files; no camera, microphone capture or camera registration is required. `cargo build --release` alone builds only the Rust executables.

| Built file | Purpose |
|---|---|
| `target/release/open-eos-studio.exe` | Launchable Studio window |
| `target/release/eos-camera.exe` | Diagnostic / recording CLI |
| `build/vcam/Release/OpenEosCameraSource.dll` | Windows virtual-camera source |
| `build/vcam/Release/eos-vcam.exe` | Native source diagnostic controller |

To share the newly built feed, run `./scripts/install-camera.ps1` once, accept Windows elevation, then launch the GUI. Close running copies before rebuilding. To make a distributable ZIP from a committed checkout, run `./scripts/package.ps1 -Destination ../Open-EOS-Studio-local`; this builds/tests both components and includes source and license notices.

See [docs/building.md](docs/building.md) for exact setup, smaller development commands, packaging, release automation and common compiler/runtime errors. GitHub Actions builds/tests each push and PR; version tags build and publish preview release assets only after checks pass.

## Use the camera diagnostic application

Connect the T3i by USB, switch it on and keep it awake. Close camera clients. Canon's `EWCService` and `EWCPairingService` may still be accessing the camera after its window closes; temporarily stop those services if connection or frame retrieval conflicts occur. This program does not stop services or change their startup settings.

```powershell
./target/release/eos-camera.exe list
./target/release/eos-camera.exe probe
./target/release/eos-camera.exe status
./target/release/eos-camera.exe preview --output captures/landscape.jpg
./target/release/eos-camera.exe preview --rotate 90 --output captures/portrait.jpg
./target/release/eos-camera.exe benchmark --frames 120 --rotate 90
./target/release/eos-camera.exe microphones
./target/release/eos-camera.exe record --seconds 10 --rotate 90 --crop portrait --microphone default --output captures/test.mp4
./target/release/eos-camera.exe record --seconds 10 --crop landscape --full-hd --microphone default --output captures/1080.mp4
```

`record` uses the same capture/recording worker as the GUI. `--microphone` also accepts `off` or an exact quoted input name. `--test-pattern` substitutes generated moving pixels for the camera, useful for recording tests; it still captures the selected microphone unless `--microphone off` is specified. Ctrl+C requests a clean stop.

Saved previews are live-view images, **not full-resolution still photos**. Non-rotated previews preserve the original USB JPEG bytes; rotated previews are decoded and re-encoded at JPEG quality 95. Output files are never overwritten. Images and build output are ignored by Git.

Experimental focus commands require a compatible electronically controlled lens, normally with its switch set to AF:

```powershell
./target/release/eos-camera.exe focus near --step 1
./target/release/eos-camera.exe focus far --step 1
./target/release/eos-camera.exe autofocus
```

Steps 1–3 are Canon's relative movement sizes, not distances. Autofocus runs for a bounded observation period and is then cancelled; it does not yet interpret focus-lock events. A focus command is never automatically repeated after an uncertain reply. The camera's physical mode dial remains authoritative.

If Near/Far is too subtle, choose **Large (3)** and check that the lens switch is **AF**; MF disables remote motor control. Commands are available before local recording. Studio reports the camera's AF method (Quick, Live or face detection on the T3i), which is separate from exposure metering. Point positioning and changing the AF method from Studio are not implemented.

The app serializes its own camera access within the Windows user session. It cannot coordinate competing Canon or third-party applications. Disconnects produce errors. Normal cleanup restores only live-view properties this process changed; if USB is removed or the process is killed, switch the camera off/on to reset it. Individual WPD calls can block according to the Windows driver's timeout, even though application retries are bounded.

## Optional synthetic source diagnostic

The build runs the DLL streaming test without registering a device. To make the **animated test pattern** selectable, first register the DLL using the installer above. The separate controller explicitly enables a test-pattern attribute; the Studio feed never silently falls back to a pattern.

From an Administrator PowerShell:

```powershell
./scripts/register-test-camera.ps1
```

The script copies the camera DLL and controller to a protected directory under Program Files and registers only this project's COM class. It does not replace the Canon camera's driver. Then, from a normal terminal:

```powershell
& "$env:ProgramFiles/Open EOS Camera Development/eos-vcam.exe" run
```

Select **Open EOS Camera (test pattern)** in a camera app. Press Enter in the controller to stop/remove the session device. The test source has no microphone. To remove the COM registration, stop the controller and run `./scripts/register-test-camera.ps1 -Unregister` from Administrator PowerShell. The script leaves installed files in place for deliberate removal after all clients exit.

**OBS real-feed preview and recording are verified; Zoom, Meet, browser and Windows Camera compatibility remain unverified.** A source-reader test alone does not establish client compatibility.

## Next development slice

1. Resolve simultaneous-consumer failures and validate Zoom, Meet, browsers and Windows Camera against the registered feed.
2. Add persistent portrait/landscape profiles to the desktop preview and recorder.
3. Add a signed production installer and improve reconnect behavior.
4. Add original-photo capture/download and verified exposure/focus controls. Preserve originals independently of preview transforms.

## License and provenance

Project code is GPL-3.0-or-later. The Windows source adapter is adapted from Microsoft's MIT-licensed Windows-Camera sample; its original notices are retained. Canon protocol layouts and sequencing were developed using libgphoto2's LGPL-2.1-or-later implementation as a reference. See [protocol and dependency sources](docs/protocol-sources.md), LICENSE, LICENSES and the native adapter's third_party directory. This project is independent of Canon.
