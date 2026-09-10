Open EOS Studio can preview and record a Canon EOS 600D / Rebel T3i over its existing Windows USB driver, then share the rotated video as **Open EOS Camera (Windows Virtual Camera)**.

### Download and run

Download **Open-EOS-Studio-0.3.0-windows-x64.zip** from this release's Assets and extract the entire folder. GitHub's automatically generated "Source code" downloads contain source, not the runnable app.

Requires **Windows 11 x64**, the current [Microsoft Visual C++ x64 Redistributable](https://aka.ms/vc14/vc_redist.x64.exe), Windows media components and an OpenGL 3.3-capable graphics driver. Windows N editions also need Microsoft's Media Feature Pack. No compiler, Canon Webcam Utility, Canon SDK, Python or FFmpeg installation is needed to run the app.

Run **Install virtual camera.cmd** once if you want to use the camera in OBS/other apps. Accept the Windows administrator prompt, then open **open-eos-studio.exe** and click **Start virtual camera**. Select the Windows camera and your separate microphone in OBS. Local Studio recording does not require camera registration. Read START-HERE.md and README.md in the ZIP.

### Included and verified

- Native USB live view, rotation and portrait/landscape cropping.
- Local H.264 MP4 recording with optional separate-microphone AAC audio.
- Windows virtual camera with six resolutions in NV12/RGB32.
- OBS Studio 32.0.2 upright 704 × 1056 preview and recording, including Yeti microphone audio, verified on Windows 11 Home build 26200.
- Source archive, build guide, registration scripts and dependency license notices.
- Hosted build, Rust tests, twelve native streaming formats and cross-process frame transport checks run before publication.

### Preview limitations

This is an unsigned development preview. Use **one camera-consuming app at a time**; mixed-resolution simultaneous consumers failed during testing. Zoom, Meet, browsers and Windows Camera have not been individually validated. USB live view was measured at 1056 × 704, with roughly 18–20 unique frames per second in short local tests; a 30 fps output repeats frames and does not provide native 1080p detail. Full-resolution still capture, exposure/mode controls, persistent settings and long-session audio drift correction remain future work.

The SHA-256 companion file covers the downloadable ZIP. The `source.zip` inside it identifies the exact source commit; see BUILDING.md for compiler prerequisites and build commands.
