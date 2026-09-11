Open EOS Studio 0.3.1 improves framing, focus controls and streaming performance options for the Canon EOS 600D / Rebel T3i on Windows 11 x64.

### Changes

- Starts upright at **0° landscape**, replacing the old 90° default. Landscape, Portrait / mobile and Rotate 90° controls change the actual exported pixels.
- **1920 × 1080 landscape** and **1080 × 1920 portrait** output for recording and the Windows virtual camera. These upscale the measured 1056 × 704 USB feed; they add no native camera detail. Native USB quality remains available.
- **Show Studio preview** toggle: hide the preview to stop texture uploads and reduce redraws while capture, recording and virtual-camera output continue. Minimizing also pauses image uploads.
- Near/Far controls offer Small, Medium and Large steps, defaulting to Large. Lens movement with step 3 was confirmed on the development camera. The lens switch must remain in AF.
- Autofocus continues requesting video during its bounded request; the hardware test retrieved 29 frames. Camera AF method and lens MF status are shown.
- Experimental preview-only focus markers when the camera supplies valid, recent positions. The tested T3i currently supplies none in Live AF mode, so visible marker alignment is not yet verified. Selected markers do not establish focus lock.

### Download and update

Download **Open-EOS-Studio-0.3.1-windows-x64.zip** from Assets and extract the entire folder. GitHub's automatic Source code archives do not include executables.

Requires **Windows 11 x64**, the [Microsoft Visual C++ x64 Redistributable](https://aka.ms/vc14/vc_redist.x64.exe), Windows media components and an OpenGL 3.3-capable graphics driver. Windows N editions also need the Media Feature Pack. No Canon SDK, Webcam Utility, Python, FFmpeg or compiler is needed to run it. Compiler prerequisites and build commands are included in BUILDING.md and README.md.

Close Studio and all camera clients, then run **Install virtual camera.cmd** again to update the available resolutions. If Windows retains the old DLL, restart Windows and install before opening camera clients. Open **open-eos-studio.exe**, then **Start virtual camera**. In OBS select **Open EOS Camera (Windows Virtual Camera)** and custom 1920 × 1080 or 1080 × 1920. **Reset any compensating 270° OBS/TikTok rotation to 0°.** Choose the separate microphone in your recording/streaming application. Keep Studio open; hiding its preview is safe, closing it stops the camera.

### Validation and remaining limits

The real T3i recorded a 1920 × 1080 H.264 MP4 with Yeti AAC audio: 135 frames over 8.085 seconds, both streams decoded successfully, and audio/video end times were within 12 ms. This short test does not establish perceptual lip-sync or long-session drift correction. The registered Windows camera also delivered moving 1080 landscape video with Studio's preview hidden and 1080 portrait video to a DirectShow consumer.

The build checks 20 Rust tests, all 16 native source formats, and cross-process RGB transport/offline clearing at native portrait and both 1080 dimensions. These checks run before release publication.

This remains an unsigned development preview. Use one camera-consuming application at a time; mixed-resolution simultaneous consumers are not reliable. OBS was previously verified; Zoom, Meet, TikTok, browsers and Windows Camera still need individual validation. Full-resolution still capture, exposure/mode controls, persistent settings and long-session audio drift correction remain future work. Source, licenses and a SHA-256 checksum accompany the release.
