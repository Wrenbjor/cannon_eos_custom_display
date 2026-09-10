use crate::{
    audio::{self, Microphone},
    bridge::{Publisher, VirtualCamera},
    eos::Session,
    frame::{Aspect, Frame},
    recording::Recording,
    wpd::{Camera, Com},
};
use anyhow::{Context, Result};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Disconnected,
    Connecting,
    Preview,
    Starting,
    Recording,
    Saving,
    Closed,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub rotation: u16,
    pub aspect: Aspect,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            rotation: 90,
            aspect: Aspect::Native,
        }
    }
}
#[derive(Clone)]
pub struct State {
    requested_settings: Settings,
    pub phase: Phase,
    pub frame: Option<Arc<Frame>>,
    pub sequence: u64,
    pub fps: f64,
    pub elapsed: f64,
    pub mic_level: f32,
    pub microphones: Vec<String>,
    pub last_file: Option<PathBuf>,
    pub error: Option<String>,
    pub test_pattern: bool,
    pub virtual_camera: bool,
}
pub enum Command {
    Connect,
    Configure(Settings),
    Start(PathBuf, Microphone),
    Stop,
    Focus(bool),
    Autofocus,
    VirtualCamera(bool),
}
pub struct Controller {
    state: Arc<Mutex<State>>,
    sender: SyncSender<Command>,
    quit: Arc<AtomicBool>,
}
impl Controller {
    pub fn new(test_pattern: bool, settings: Settings) -> Self {
        let state = Arc::new(Mutex::new(State {
            requested_settings: settings,
            phase: Phase::Connecting,
            frame: None,
            sequence: 0,
            fps: 0.0,
            elapsed: 0.0,
            mic_level: 0.0,
            microphones: vec![],
            last_file: None,
            error: None,
            test_pattern,
            virtual_camera: false,
        }));
        let (sender, receiver) = mpsc::sync_channel(16);
        let quit = Arc::new(AtomicBool::new(false));
        let (thread_state, thread_quit) = (state.clone(), quit.clone());
        thread::spawn(move || {
            let _com = match Com::initialize() {
                Ok(com) => com,
                Err(e) => {
                    let mut s = thread_state.lock().unwrap();
                    s.error = Some(format!("{e:#}"));
                    s.phase = Phase::Closed;
                    return;
                }
            };
            match audio::inputs() {
                Ok(names) => thread_state.lock().unwrap().microphones = names,
                Err(e) => {
                    thread_state.lock().unwrap().error =
                        Some(format!("Microphone list unavailable: {e:#}"))
                }
            }
            let mut settings = settings;
            loop {
                if thread_quit.load(Ordering::Relaxed) {
                    break;
                }
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(Command::Connect) => {
                        {
                            let mut s = thread_state.lock().unwrap();
                            s.phase = Phase::Connecting;
                            s.error = None;
                        }
                        let result = capture_loop(
                            &_com,
                            &thread_state,
                            &receiver,
                            &thread_quit,
                            test_pattern,
                            &mut settings,
                        );
                        let mut s = thread_state.lock().unwrap();
                        s.frame = None;
                        s.phase = Phase::Disconnected;
                        if let Err(e) = result {
                            s.error = Some(format!("{e:#}"));
                        }
                    }
                    Ok(Command::Configure(value)) => settings = value,
                    Ok(_) => {
                        let mut s = thread_state.lock().unwrap();
                        s.phase = Phase::Disconnected;
                        s.error = Some("Connect the camera before recording.".into());
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            thread_state.lock().unwrap().phase = Phase::Closed;
        });
        let controller = Self {
            state,
            sender,
            quit,
        };
        let _ = controller.send(Command::Connect);
        controller
    }
    pub fn snapshot(&self) -> State {
        self.state.lock().unwrap().clone()
    }
    pub fn send(&self, command: Command) -> Result<()> {
        let next = match &command {
            Command::Connect => Some(Phase::Connecting),
            Command::Start(..) => Some(Phase::Starting),
            Command::Stop => Some(Phase::Saving),
            _ => None,
        };
        // Update while holding the lock so a fast worker cannot have its reply overwritten.
        let mut state = self.state.lock().unwrap();
        match &command {
            Command::Start(..) => anyhow::ensure!(
                state.phase == Phase::Preview && state.frame.is_some(),
                "Wait for live preview before recording"
            ),
            Command::Stop => {
                anyhow::ensure!(state.phase == Phase::Recording, "No recording is running")
            }
            Command::Configure(_) => anyhow::ensure!(
                matches!(state.phase, Phase::Preview | Phase::Disconnected),
                "Wait until recording or camera setup finishes before changing framing"
            ),
            _ => {}
        }
        let configuring =
            matches!(&command, Command::Configure(_)) && state.phase == Phase::Preview;
        let requested = if let Command::Configure(settings) = &command {
            Some(*settings)
        } else {
            None
        };
        self.sender
            .try_send(command)
            .context("Camera command queue is busy")?;
        if let Some(settings) = requested {
            state.requested_settings = settings;
        }
        if let Some(phase) = next {
            state.phase = phase;
        }
        if configuring {
            state.phase = Phase::Connecting;
            state.frame = None;
        }
        Ok(())
    }
    pub fn shutdown(&self) {
        self.quit.store(true, Ordering::Relaxed);
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn save_recording(recording: &mut Option<Recording>, state: &Arc<Mutex<State>>) {
    if let Some(active) = recording.take() {
        state.lock().unwrap().phase = Phase::Saving;
        let result = active.finish();
        let mut s = state.lock().unwrap();
        match result {
            Ok(path) => s.last_file = Some(path),
            Err(e) => s.error = Some(format!("Recording could not be completed: {e:#}")),
        }
        s.phase = Phase::Preview;
        s.elapsed = 0.0;
        s.mic_level = 0.0;
    }
}

fn capture_loop(
    com: &Com,
    state: &Arc<Mutex<State>>,
    commands: &Receiver<Command>,
    quit: &AtomicBool,
    test_pattern: bool,
    settings: &mut Settings,
) -> Result<()> {
    let camera = if test_pattern {
        None
    } else {
        Some(Camera::open(com)?)
    };
    let mut session = camera.as_ref().map(Session::start).transpose()?;
    if let Some(s) = session.as_mut() {
        s.start_live_view()?;
    }
    let mut recording: Option<Recording> = None;
    // Keep the virtual-camera object alive before dropping its pipe publisher.
    let mut publisher: Option<Publisher> = None;
    let mut virtual_camera: Option<VirtualCamera> = None;
    let mut last: Option<Arc<Frame>> = None;
    let mut keep_awake = Instant::now();
    let mut fps_epoch = Instant::now();
    let mut fps_frames = 0u32;
    let mut sequence = 0;
    let result = (|| -> Result<()> {
        while !quit.load(Ordering::Relaxed) {
            for command in commands.try_iter() {
                match command {
                    Command::Connect => {}
                    Command::VirtualCamera(enabled) => {
                        if enabled && virtual_camera.is_none() {
                            let result = (|| -> Result<(Publisher, VirtualCamera)> {
                                let mut publisher = Publisher::new()?;
                                publisher.publish(last.as_deref())?;
                                let camera = VirtualCamera::start(&publisher.name)?;
                                Ok((publisher, camera))
                            })();
                            match result {
                                Ok((p, c)) => {
                                    publisher = Some(p);
                                    virtual_camera = Some(c);
                                    state.lock().unwrap().virtual_camera = true;
                                }
                                Err(e) => {
                                    state.lock().unwrap().error =
                                        Some(format!("Virtual camera: {e:#}"))
                                }
                            }
                        } else if !enabled {
                            if let Some(p) = &mut publisher {
                                let _ = p.publish(None);
                            }
                            virtual_camera.take();
                            publisher.take();
                            state.lock().unwrap().virtual_camera = false;
                        }
                    }
                    Command::Configure(value) if recording.is_none() => {
                        if let Some(p) = &mut publisher {
                            p.publish(None)?;
                        }
                        *settings = value;
                        last = None;
                    }
                    Command::Configure(_) => {}
                    Command::Start(path, microphone) if recording.is_none() => {
                        let result = last
                            .clone()
                            .context("Wait for the first preview frame")
                            .and_then(|frame| Recording::start(&path, frame, &microphone));
                        let mut s = state.lock().unwrap();
                        match result {
                            Ok(active) => {
                                recording = Some(active);
                                s.phase = Phase::Recording;
                                s.error = None;
                                s.elapsed = 0.0;
                            }
                            Err(e) => {
                                s.phase = Phase::Preview;
                                s.error = Some(format!("Could not start recording: {e:#}"));
                            }
                        }
                    }
                    Command::Start(..) => {}
                    Command::Stop => save_recording(&mut recording, state),
                    Command::Focus(far) if recording.is_none() => {
                        if let Some(s) = &session
                            && let Err(e) = s.focus_step(far, 1)
                        {
                            state.lock().unwrap().error = Some(format!("Focus: {e:#}"));
                        }
                    }
                    Command::Autofocus if recording.is_none() => {
                        if let Some(s) = &mut session
                            && let Err(e) = s.autofocus()
                        {
                            state.lock().unwrap().error = Some(format!("Autofocus: {e:#}"));
                        }
                    }
                    _ => {}
                }
            }
            if quit.load(Ordering::Relaxed) {
                break;
            }
            let (frame, captured) = if let Some(s) = &mut session {
                if keep_awake.elapsed() >= Duration::from_secs(5) {
                    s.keep_awake()?;
                    keep_awake = Instant::now();
                }
                let jpeg = s.frame()?;
                let captured = Instant::now();
                (Frame::decode(&jpeg)?, captured)
            } else {
                thread::sleep(Duration::from_millis(33));
                (test_frame(sequence), Instant::now())
            };
            let frame = Arc::new(frame.rotate(settings.rotation)?.crop(settings.aspect)?);
            if let Some(p) = &mut publisher {
                p.publish(Some(&frame))?;
            }
            let record_error = if let Some(active) = &mut recording {
                active.push(frame.clone(), captured).err()
            } else {
                None
            };
            if let Some(e) = record_error {
                state.lock().unwrap().error = Some(format!("Recording stopped: {e:#}"));
                save_recording(&mut recording, state);
            }
            sequence += 1;
            fps_frames += 1;
            let mut s = state.lock().unwrap();
            // A settings command may arrive while the previous frame is being
            // fetched. Never advertise that old frame as the updated preview.
            if s.requested_settings != *settings {
                continue;
            }
            if s.phase == Phase::Connecting {
                s.phase = Phase::Preview;
            }
            s.sequence += 1;
            s.frame = Some(frame.clone());
            if fps_epoch.elapsed().as_secs_f64() >= 1.0 {
                s.fps = f64::from(fps_frames) / fps_epoch.elapsed().as_secs_f64();
                fps_epoch = Instant::now();
                fps_frames = 0;
            }
            if let Some(active) = &recording {
                s.elapsed = active.epoch.elapsed().as_secs_f64();
                s.mic_level = active.peak;
            }
            last = Some(frame);
        }
        Ok(())
    })();
    if let Some(p) = &mut publisher {
        let _ = p.publish(None);
    }
    virtual_camera.take();
    publisher.take();
    state.lock().unwrap().virtual_camera = false;
    save_recording(&mut recording, state);
    let cleanup = session.as_mut().map(Session::stop).transpose();
    result?;
    cleanup?;
    Ok(())
}

pub fn test_frame(index: u64) -> Frame {
    let (width, height) = (640usize, 480usize);
    let mut rgb = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let color = if y < 32 {
                [255, 255, 255]
            } else if x < 32 {
                [255, 30, 30]
            } else {
                [
                    (x * 255 / width) as u8,
                    (y * 255 / height) as u8,
                    if x.abs_diff((index as usize * 7) % width) < 20 {
                        255
                    } else {
                        24
                    },
                ]
            };
            rgb.extend_from_slice(&color);
        }
    }
    Frame {
        width: width as u16,
        height: height as u16,
        rgb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait(controller: &Controller, phase: Phase) -> Result<State> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let state = controller.snapshot();
            if let Some(error) = &state.error {
                anyhow::bail!("{error}");
            }
            if state.phase == phase {
                return Ok(state);
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "Timed out waiting for {phase:?}; got {:?}",
                state.phase
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn changing_framing_then_recording_and_closing_finalizes_files() -> Result<()> {
        let controller = Controller::new(true, Settings::default());
        let result = (|| -> Result<()> {
            wait(&controller, Phase::Preview)?;
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "open-eos-controller-{}-{unique}.mp4",
                std::process::id()
            ));
            controller.send(Command::Configure(Settings {
                rotation: 0,
                aspect: Aspect::Landscape,
            }))?;
            let preview = wait(&controller, Phase::Preview)?.frame.unwrap();
            assert_eq!((preview.width, preview.height), (640, 360));
            controller.send(Command::Start(path.clone(), Microphone::Off))?;
            wait(&controller, Phase::Recording)?;
            assert!(
                controller
                    .send(Command::Configure(Settings::default()))
                    .is_err()
            );
            thread::sleep(Duration::from_millis(300));
            controller.send(Command::Stop)?;
            let saved = wait(&controller, Phase::Preview)?;
            assert_eq!(saved.last_file.as_ref(), Some(&path));
            assert!(std::fs::read(&path)?.windows(4).any(|b| b == b"moov"));
            assert!(!path.with_extension("recording.mp4").exists());
            std::fs::remove_file(&path)?;
            // Closing during a second recording must finish it before Closed.
            controller.send(Command::Start(path.clone(), Microphone::Off))?;
            wait(&controller, Phase::Recording)?;
            thread::sleep(Duration::from_millis(200));
            controller.shutdown();
            wait(&controller, Phase::Closed)?;
            assert!(std::fs::read(&path)?.windows(4).any(|b| b == b"moov"));
            std::fs::remove_file(path)?;
            Ok(())
        })();
        controller.shutdown();
        let cleanup = wait(&controller, Phase::Closed);
        result?;
        cleanup?;
        Ok(())
    }
}
