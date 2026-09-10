#![windows_subsystem = "windows"]

fn main() {
    let auto_camera = std::env::args().any(|arg| arg == "--virtual-camera");
    if let Err(error) = eos_camera::gui::run_with_virtual_camera(auto_camera) {
        use windows::{
            Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
            core::{HSTRING, w},
        };
        unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(format!("Could not open EOS Studio:\n{error:#}")),
                w!("Open EOS Studio"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
