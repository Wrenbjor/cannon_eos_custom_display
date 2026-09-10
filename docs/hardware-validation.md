# Initial validation — September 10, 2026

Development baseline: Windows 11 Home x64, build 26200; AMD Ryzen 7 5700G; physical Canon EOS Rebel T3i, USB VID 04A9 / PID 3218. USB serial and owner fields are deliberately omitted. Firmware version, lens model and power configuration have not yet been recorded.

## Observed on the real camera

| Check | Result |
|---|---|
| Enumerate physical camera through Windows Portable Devices | Passed |
| Open using existing WPD driver, without Canon EDSDK or replacing a driver | Passed |
| Read vendor command list | Passed; live view, autofocus, lens drive and remote shutter advertised |
| Read selected properties through EOS events | Passed |
| Retrieve native USB preview JPEG | Passed: 1056 × 704, 205,518 bytes for the first frame |
| Decode and rotate live frames 90° clockwise | Passed: 704 × 1056 output |
| Save and inspect a rotated real-camera JPEG | Passed: upright 704 × 1056 image, independent of rotation metadata |
| Optimized 120-frame benchmark, decoding + rotation included | Passed: 6.1564 seconds, approximately 19.49 delivered fps |
| Distinct payloads during that benchmark | 120 of 120 JPEG payloads differed; this is not a sensor-fps or motion-estimation measurement |
| Near-focus step 1 and far-focus step 1 | Camera accepted both commands; optical movement/accuracy not measured |
| Autofocus lock | Not tested; implementation does not yet parse focus-lock events |
| Remote shutter / original JPEG / CR2 transfer | Not implemented |

One native JPEG was saved locally for inspection. No captured image, USB serial, owner field or private device identifier is committed to the repository. The benchmark saved no images. Its measured rate is for this camera/machine/mode and short run, not a sustained performance guarantee or evidence of native 1080p USB delivery.

## What the hardware tests revealed

1. Session setup can briefly return camera-busy responses. Only retryable state-setting/read operations have bounded retries; lens movement is not repeated automatically.
2. Canon's EWCService and EWCPairingService were active during initial failures. The user stopped them and restarted the camera before successful frame acquisition. This supports a contention diagnosis; it does not prove those services caused every earlier error.
3. The camera reported EVF mode 2 in the tested setup and rejected setting mode 1 with DeviceBusy. Preview still worked. Cleanup now restores only settings the process actually changed, so it does not attempt to restore an untouched movie-mode property.
4. The camera disappeared between tests and reappeared after the user confirmed it was on/awake. Automatic reconnection and power-state diagnosis are future work.

## Windows source verification

The native DLL builds with MSVC 19.44 and Windows SDK 10.0.26100.0. A Media Foundation source reader activates it directly through its COM class factory, reads 30 frames in each of four formats, checks pixel changes and increasing timestamps, then shuts down the source. All four format tests passed locally: 1280 × 720 NV12, 1280 × 720 RGB32, 720 × 1280 NV12, and 720 × 1280 RGB32.

This is **an in-process synthetic camera-source test**. It does not test device registration, Windows Frame Server hosting, IPC, multiple consumers or a real camera application. The source currently displays an animated test pattern. The elevated development registration script has been syntax-checked but has not been executed on the development machine.

| Required client | Registered test source | Real USB camera feed |
|---|---|---|
| Zoom desktop | Not tested | Not connected |
| Google Meet in Chrome | Not tested | Not connected |
| Google Meet in Edge | Not tested | Not connected |
| Chrome / Edge / Firefox camera APIs | Not tested | Not connected |
| OBS ordinary Video Capture Device | Not tested | Not connected |
| Windows Camera | Not tested | Not connected |

## Automated checks

Rust tests cover truncated/oversized Canon records, invalid terminators, trailing corruption after a JPEG, non-JPEG payload rejection, exact pixel rotation, four-turn identity, invalid angles, JPEG decode failure and encoded portrait dimensions. The build script also runs rustfmt and Clippy with warnings denied.

No hardware-dependent command runs in CI. Native streaming tests have a 30-second process timeout. The Windows build workflow is included; local success does not imply a hosted workflow has run successfully.

The first hosted run compiled the Rust executable but exposed C++/WinRT's deprecated experimental-coroutine path under Visual Studio 2026. The native target was changed to C++20 to use standard coroutines, and local native streaming tests were rerun. The checkout action was also updated from its deprecated runtime version.

Milestone 0 remains **partially complete**: USB acquisition and synthetic source streaming are proven, while original-photo transfer, registration and application compatibility acceptance tests remain open.
