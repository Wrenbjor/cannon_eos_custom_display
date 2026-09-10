//! Versioned, bounded, local-only frame transport to the Windows camera source.
use crate::{frame::Frame, recording::Runtime};
use anyhow::{Context, Result, ensure};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Foundation::*,
        Media::MediaFoundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::{
            Com::CoCreateGuid, IO::*, Pipes::*, SystemInformation::GetTickCount64, Threading::*,
        },
    },
    core::{GUID, HSTRING, PWSTR, w},
};

pub const PIPE_ATTRIBUTE: GUID = GUID::from_u128(0x921f5ac1_3d6b_4ff6_8e31_2356a63aa113);
pub fn self_test(reader: &std::path::Path, source: &std::path::Path) -> Result<()> {
    let mut publisher = Publisher::new()?;
    let frame = Frame {
        width: 64,
        height: 96,
        rgb: [12, 34, 56].repeat(64 * 96),
    };
    for mode in ["live", "offline"] {
        publisher.publish(if mode == "live" { Some(&frame) } else { None })?;
        let mut child = std::process::Command::new(reader)
            .arg("bridge-test")
            .arg(std::path::absolute(source)?)
            .arg(&publisher.name)
            .arg(mode)
            .spawn()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait()? {
                ensure!(status.success(), "Native bridge verification failed");
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("Native bridge verification timed out");
            }
            // Refresh the producer heartbeat while the child initializes MF.
            publisher.publish(if mode == "live" { Some(&frame) } else { None })?;
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
    Ok(())
}
const MAX_PAYLOAD: usize = 16 * 1024 * 1024;
struct Handle(HANDLE);
// Handles are transferred to a single owning pipe thread and never shared.
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn packet(frame: Option<&Frame>, sequence: u64) -> Result<Vec<u8>> {
    let (width, height, payload) = frame
        .map(|f| (u32::from(f.width), u32::from(f.height), f.rgb.as_slice()))
        .unwrap_or((0, 0, &[]));
    ensure!(
        payload.len() <= MAX_PAYLOAD && payload.len() == width as usize * height as usize * 3,
        "Invalid shared video frame"
    );
    let mut bytes = Vec::with_capacity(40 + payload.len());
    for n in [
        0x31534f45u32,
        1,
        width,
        height,
        payload.len() as u32,
        u32::from(frame.is_some()),
    ] {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.extend_from_slice(&unsafe { GetTickCount64() }.to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

fn user_sid() -> Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let token = Handle(token);
        let mut needed = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
        ensure!(needed > 0 && needed < 65536, "Unexpected user token size");
        let mut storage = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )?;
        let user = &*storage.as_ptr().cast::<TOKEN_USER>();
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text)?;
        let result = text.to_string();
        LocalFree(Some(HLOCAL(text.0.cast())));
        Ok(result?)
    }
}

fn complete(handle: HANDLE, overlap: &OVERLAPPED) -> Result<u32> {
    unsafe {
        let mut count = 0;
        let result = GetOverlappedResultEx(handle, overlap, &mut count, 200, false);
        if result.is_err() {
            let _ = CancelIoEx(handle, Some(overlap));
            // Keep buffers and OVERLAPPED alive until cancellation completes.
            let _ = GetOverlappedResult(handle, overlap, &mut count, true);
        }
        result?;
        Ok(count)
    }
}

pub struct Publisher {
    pub name: String,
    latest: Arc<Mutex<Arc<Vec<u8>>>>,
    quit: Arc<AtomicBool>,
    workers: Vec<std::thread::JoinHandle<()>>,
    sequence: u64,
}
impl Publisher {
    pub fn new() -> Result<Self> {
        let name = format!(r"\\.\pipe\OpenEOSCamera-{:?}", unsafe { CoCreateGuid()? });
        let latest = Arc::new(Mutex::new(Arc::new(packet(None, 0)?)));
        let quit = Arc::new(AtomicBool::new(false));
        let mut workers = Vec::new();
        unsafe {
            let sddl = HSTRING::from(format!(
                "D:P(A;;GA;;;{})(A;;GRGW;;;SY)(A;;GRGW;;;LS)",
                user_sid()?
            ));
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &sddl,
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: false.into(),
            };
            let mut handles = Vec::new();
            for index in 0..4 {
                let flags = PIPE_ACCESS_DUPLEX
                    | FILE_FLAG_OVERLAPPED
                    | if index == 0 {
                        FILE_FLAG_FIRST_PIPE_INSTANCE
                    } else {
                        FILE_FLAGS_AND_ATTRIBUTES(0)
                    };
                let handle = CreateNamedPipeW(
                    &HSTRING::from(&name),
                    flags,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    4,
                    65536,
                    64,
                    200,
                    Some(&attributes),
                );
                if handle == INVALID_HANDLE_VALUE {
                    let error = windows::core::Error::from_thread();
                    LocalFree(Some(HLOCAL(descriptor.0)));
                    return Err(error.into());
                }
                handles.push(Handle(handle));
            }
            LocalFree(Some(HLOCAL(descriptor.0)));
            for pipe in handles {
                let (latest, quit) = (latest.clone(), quit.clone());
                workers.push(std::thread::spawn(move || {
                    let pipe = pipe; // Transfer the owning wrapper, not its raw HANDLE field.
                    while !quit.load(Ordering::Relaxed) {
                        let mut connect = OVERLAPPED::default();
                        let connected = match ConnectNamedPipe(pipe.0, Some(&mut connect)) {
                            Ok(()) => true,
                            Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => true,
                            Err(e) if e.code() == ERROR_IO_PENDING.to_hresult() => {
                                complete(pipe.0, &connect).is_ok()
                            }
                            _ => false,
                        };
                        if connected && !quit.load(Ordering::Relaxed) {
                            let frame = latest.lock().unwrap().clone();
                            let mut write = OVERLAPPED::default();
                            let result = WriteFile(pipe.0, Some(&frame), None, Some(&mut write));
                            if (result.is_ok()
                                || result.is_err_and(|e| e.code() == ERROR_IO_PENDING.to_hresult()))
                                && complete(pipe.0, &write).is_ok()
                            {
                                // Client acknowledges after copying the full frame. Disconnecting
                                // before that would discard unread pipe bytes. This wait is bounded.
                                let mut ack = [0u8];
                                let mut read = OVERLAPPED::default();
                                let result =
                                    ReadFile(pipe.0, Some(&mut ack), None, Some(&mut read));
                                if result.is_ok()
                                    || result
                                        .is_err_and(|e| e.code() == ERROR_IO_PENDING.to_hresult())
                                {
                                    let _ = complete(pipe.0, &read);
                                }
                            }
                        }
                        let _ = DisconnectNamedPipe(pipe.0);
                    }
                }));
            }
        }
        Ok(Self {
            name,
            latest,
            quit,
            workers,
            sequence: 0,
        })
    }
    pub fn publish(&mut self, frame: Option<&Frame>) -> Result<()> {
        self.sequence += 1;
        *self.latest.lock().unwrap() = Arc::new(packet(frame, self.sequence)?);
        Ok(())
    }
}
impl Drop for Publisher {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

pub struct VirtualCamera {
    camera: IMFVirtualCamera,
    _runtime: Runtime,
}
impl VirtualCamera {
    pub fn start(pipe: &str) -> Result<Self> {
        let runtime = Runtime::start()?;
        unsafe {
            let camera = MFCreateVirtualCamera(
                MFVirtualCameraType_SoftwareCameraSource,
                MFVirtualCameraLifetime_Session,
                MFVirtualCameraAccess_CurrentUser,
                w!("Open EOS Camera"),
                w!("{EAD49F23-8F8C-47E7-A78C-F7744A6D54C9}"),
                None,
            )?;
            camera.SetString(&PIPE_ATTRIBUTE, &HSTRING::from(pipe))?;
            if let Err(e) = camera.Start(None::<&IMFAsyncCallback>) {
                let _ = camera.Remove();
                let _ = camera.Shutdown();
                return Err(e).context("Starting the virtual camera. Run Install virtual camera.ps1 once, then allow camera access in Windows Settings");
            }
            Ok(Self {
                camera,
                _runtime: runtime,
            })
        }
    }
}
impl Drop for VirtualCamera {
    fn drop(&mut self) {
        unsafe {
            let _ = self.camera.Stop();
            let _ = self.camera.Remove();
            let _ = self.camera.Shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packet_layout_and_disconnect_are_unambiguous() {
        let frame = Frame {
            width: 2,
            height: 2,
            rgb: vec![42; 12],
        };
        let bytes = packet(Some(&frame), 17).unwrap();
        assert_eq!(&bytes[..8], b"EOS1\x01\0\0\0");
        assert_eq!(bytes.len(), 52);
        assert_eq!(&bytes[24..32], &17u64.to_le_bytes());
        assert_eq!(&bytes[40..], &[42; 12]);
        let blank = packet(None, 18).unwrap();
        assert_eq!(blank.len(), 40);
        assert_eq!(&blank[8..24], &[0; 16]);
    }
}
