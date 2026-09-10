# Protocol and dependency sources

These are implementation references, not evidence that every camera feature works on the T3i.

## Canon EOS protocol

- [libgphoto2 PTP implementation, pinned revision](https://github.com/gphoto/libgphoto2/tree/5672d510447bed26aa4c5d646665e768281bc680/camlibs/ptp2): `ptp.h` defines operation/property codes; `ptp.c` defines transaction parameters; `ptp-pack.c` documents event records; `config.c` and `library.c` implement session setup and preview acquisition. LGPL-2.1-or-later. Copyright the libgphoto2 contributors; the upstream license is retained in LICENSES/LIBGPHOTO2.txt. Canon record layouts and command sequencing in src/protocol.rs and src/eos.rs draw on this work.
- [digiCamControl](https://github.com/dukus/digiCamControl): additional evidence that Canon vendor commands can use Windows' existing portable-device path. Its CanonBase implementation was consulted during research; it is not a runtime dependency and its source is not vendored here.
- [Microsoft WPD MTP extensions](https://learn.microsoft.com/en-us/windows/win32/wpd_sdk/supporting-mtp-extensions): Windows owns the underlying PTP session. Do not send raw USB PTP containers or open a separate PTP session through this transport.
- [Read data command](https://learn.microsoft.com/en-us/windows/win32/wpd_sdk/wpd-command-mtp-ext-read-data): requires an input buffer as well as the requested byte count and transfer context.
- [End transfer command](https://learn.microsoft.com/en-us/windows/win32/wpd_sdk/wpd-command-mtp-ext-end-data-transfer): complete every started transfer, including application validation failures, and inspect the camera response.

Implemented operation codes: SetRemoteMode 0x9114, SetEventMode 0x9115, GetEvent 0x9116, SetDevicePropValueEx 0x9110, KeepDeviceOn 0x911D, GetViewFinderData 0x9153, DoAf 0x9154, DriveLens 0x9155, AfCancel 0x9160. EVFOutputDevice is 0xD1B0 and EVFMode is 0xD1B1. Values and availability depend on camera mode and lens. Full-resolution shutter/object operations remain future work.

EOS event and viewfinder records use little-endian length/type headers. The parser validates every record before returning a JPEG, including records after it. JPEG types 1 and 11 are accepted; raw/movie type 9 is deliberately not mislabelled as JPEG. Current transfers require a known size and enforce per-operation limits. WPD unknown-length streams are not implemented.

## Windows camera source

Vendored from [microsoft/Windows-Camera](https://github.com/microsoft/Windows-Camera/tree/d348797d2f5632ff3cb250960638091c4a1a75fb/Samples/VirtualCamera/VirtualCameraMediaSource), revision `d348797d2f5632ff3cb250960638091c4a1a75fb`. Copyright Microsoft Corporation; MIT license at native/vcam/third_party/MICROSOFT-LICENSE.txt.

Local changes: a distinct COM class ID and name; a custom sample allocator usable by the direct source-reader test; landscape/portrait format descriptions; a moving, asymmetric test pattern; bounded frame production; and unlocking the pixel buffer even when frame generation fails. The sample's hardware-wrapper classes have been removed. Activation creates only this project's synthetic stream. Canon USB acquisition is implemented separately in Rust.

The adapter links Windows system libraries and the SDK's C++/WinRT headers. WIL 1.0.260126.7 is retrieved from NuGet with its SHA-256 pinned in CMakeLists.txt; its license and bundled third-party notices are in LICENSES. Rust package versions are pinned by Cargo.lock; each dependency retains its upstream license.

## Desktop preview and recording

- [eframe 0.33.3](https://docs.rs/eframe/0.33.3/eframe/) / egui provide the native desktop window and pixel preview (MIT OR Apache-2.0).
- [CPAL 0.16.0](https://docs.rs/cpal/0.16.0/cpal/) captures the selected Windows microphone using WASAPI (Apache-2.0). Audio uses a bounded callback queue and sample-count timestamps anchored to the recording clock.
- [Microsoft Sink Writer encoding tutorial](https://learn.microsoft.com/en-us/windows/win32/medfound/tutorial--using-the-sink-writer-to-encode-video) describes media types, samples and writer lifecycle.
- [Sink Writer Finalize](https://learn.microsoft.com/en-us/windows/win32/api/mfreadwrite/nf-mfreadwrite-imfsinkwriter-finalize) completes pending output and MP4 headers. The application waits for completion before publishing the final filename.
- [Microsoft AAC encoder](https://learn.microsoft.com/en-us/windows/win32/medfound/aac-encoder) defines supported PCM input and AAC output formats.

Video converts RGB to BT.709 limited-range NV12 and tags the encoded stream accordingly. Rotation and center cropping happen before both preview and encoding. MP4 uses actual capture timestamps rather than assuming the camera delivered 30 frames per second. A separate microphone has an independent hardware clock; long-session drift correction and a manual audio-delay control remain future work. No FFmpeg executable is needed by the app; FFmpeg is used only for development validation of saved files. Cargo.lock pins the dependency graph; third-party notices accompany packaged builds.

## Resolution facts

- [Canon 600D specifications](https://asia.canon/en/support/6200098100): original still-photo resolution is distinct from USB live view and camera-side movie recording.
- [Dragonframe 600D support](https://www.dragonframe.com/camera-setup/canon_eos_600d/): documents 1056 × 704 live view. This was also measured directly on the development camera.

Advertising a 1280 × 720, 720 × 1280 or future 1080p virtual output does not add native optical detail. The current Windows source emits synthetic frames, not upscaled camera video.
