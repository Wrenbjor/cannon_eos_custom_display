# Hardware validation

## Version 0.3.1 — September 11, 2026

The same T3i and Windows machine were used for this patch. The user confirmed audible/visible lens movement after one Near step 3 and one Far step 3. Studio now defaults to Large steps and offers all three sizes. Autofocus retrieved **29 live frames** during its bounded request; command completion is not proof of focus lock. The camera reported AF method **1 (Live)** and returned **no current FocusInfoEx point positions**, including after autofocus. The experimental marker parser/geometry have automated coverage, but live marker alignment remains unverified.

| Check | Result |
|---|---|
| Native USB benchmark before scaling | 45 changed JPEG payloads, 1056 × 704, 2.480 seconds, 18.15 delivered fps including decode |
| Actual T3i + Yeti local MP4 | H.264 1920 × 1080, 135 decoded frames, 8.084567 seconds (16.70 fps) |
| Yeti audio in that file | AAC, start 0.011 seconds, duration 8.085333 seconds; end within 12 ms of video |
| Complete real-camera MP4 decode | Both streams decoded without errors |
| Generated landscape + real Yeti | 1920 × 1080, 103 frames, 6.101867 seconds; audio end within 10 ms |
| Generated portrait recording | 1080 × 1920, 72 frames, 4.111100 seconds |
| Updated GUI default | Physically landscape camera visibly upright at 0°, 1920 × 1080 |
| Preview hidden + registered Windows camera | DirectShow consumer recorded moving 1920 × 1080 video; all 108 decoded frames differed; Studio remained connected |
| Portrait preset + registered Windows camera | DirectShow consumer recorded 1080 × 1920, 65 decoded frames; complete decode passed |
| Hide/show preview | Image uploads stop while hidden; live image resumes when shown |
| Native component update | Registered Program Files DLL hash matches tested build; Windows Frame Server needed restarting to release the old DLL |

Full build checks passed: **20 Rust tests**, formatting/lint, **16 native source formats** (eight dimensions in NV12/RGB32), and six cross-process checks comparing all RGB pixels and offline clearing at 704 × 1056, 1920 × 1080 and 1080 × 1920. Controller tests exercise the new default, a portrait reconfiguration, framing locks during recording and MP4 finalization.

1080 output is **upscaled USB live view**. The output frame rate depends on camera delivery and processing; this short 1080 recording was slower than the native benchmark. Timestamp checks do not establish perceptual lip-sync or long-session drift correction. OBS 0.3.0 compatibility below remains the prior baseline; this patch's new formats were tested through the registered DirectShow camera and direct Media Foundation readers. Zoom/Meet/TikTok and simultaneous consumers remain unverified. Personal test media stays local and is excluded from releases.

## Version 0.3 virtual camera / OBS

The DLL was registered under Program Files and Studio created a current-user, session-lifetime Windows virtual camera. OBS Studio **32.0.2** enumerated **Open EOS Camera (Windows Virtual Camera)** through its ordinary DirectShow Video Capture Device source. The real source preview was visually inspected: upright, in focus and **704 × 1056**.

A separate portable OBS configuration named **Open EOS Test** captured 19.854 seconds of H.264 video and stereo 48 kHz AAC from the Yeti in an MKV file. All **592 decoded video frames** and the audio stream decoded without errors. OBS output was configured at 30 fps; repeated frames are expected because actual USB acquisition was previously measured around 18–20 fps. OBS reported zero skipped encoding frames for the test. The user subsequently made another recording and reported the test successful.

Local checks pass: **16 Rust tests**, **12 native source formats** (six resolutions × NV12/RGB32), and an actual Rust-producer/native-consumer pipe test checking every RGB pixel in a 704 × 1056 output and black output after clearing the producer. Registration, Windows device enumeration and OBS consumption are now tested beyond the direct DLL test.

Known limitation: while OBS consumed the portrait feed, a second DirectShow consumer requesting 1280 × 720 failed with an I/O error. OBS continued operating. Simultaneous clients at different resolutions require investigation. Physical unplug during OBS recording and long-session lip-sync/drift remain untested. Test recordings and OBS settings stay local, outside the repository/package.

| Required client | Real USB feed, version 0.3 |
|---|---|
| OBS 32.0.2 ordinary Video Capture Device | Preview and video + separate-microphone recording passed |
| Zoom desktop | Not tested |
| Google Meet in Chrome / Edge | Not tested |
| Chrome / Edge / Firefox camera APIs | Not tested |
| Windows Camera | Not tested |

The older sections below describe previous versions.

## Version 0.2 desktop recorder

The desktop application now provides a live preview, rotation, native/9:16/16:9 center crop, a separate microphone selector, recording level meter, Start/Stop, playback and preview-time focus controls. The GUI and diagnostic `record` command use the same controller and recording implementation.

On the same Windows 11 development machine, an optimized build recorded the actual T3i USB feed with a separate Yeti microphone:

| Saved media check | Observed result |
|---|---|
| Portrait video | H.264, 594 × 1056; actual pixels rotated 90° and center-cropped to 9:16 |
| Video duration / decoded frames | 12.133667 seconds / 220 frames, approximately 18.13 delivered fps |
| Separate microphone | AAC, stereo, 48,000 Hz; audio samples were nonzero |
| Audio timeline | Starts at 0.008167 seconds, duration 12.117333 seconds; end within 9 ms of video end |
| Whole-file decode | FFmpeg decoded both streams without errors using original timestamp precision |
| Earlier short generated-pattern + real-microphone test | Both streams decoded; video timestamps strictly increasing |

These results establish valid local A/V recording, not perceptual lip-sync, sustained long-session performance, or independent hardware-clock drift correction. The camera image in the first clip was visibly out of focus; optical autofocus success remains unverified. Debug builds were substantially slower; use the optimized release executable for recording. Personal test media is kept outside Git and release archives.

Automated tests also encode generated H.264/AAC media, verify finalized MP4 tracks, refuse overwriting an existing recording, preserve crop pixels, reject changing framing during recording, and finish a second recording when the controller closes. There are 15 Rust tests. Manual USB unplug during recording, microphone removal, low-disk handling and a long A/V clap-sync run remain untested. The GUI has not been driven by an automated native UI test.

This release does **not** connect the real USB feed to the system-wide virtual camera. The application matrix below remains open.

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
