use eos_camera::{audio, eos, frame, studio, wpd};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Open EOS Camera — Canon 600D / Rebel T3i on Windows")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// List cameras without exposing USB serial numbers.
    List,
    /// Open the desktop preview and recording window.
    Studio,
    /// List separate Windows microphone inputs.
    Microphones,
    /// Record an MP4 using the same capture worker as the desktop application.
    Record {
        #[arg(long)]
        output: std::path::PathBuf,
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=86400))]
        seconds: u32,
        #[arg(long, default_value = "90", value_parser = rotation)]
        rotate: u16,
        #[arg(long, value_enum, default_value = "native")]
        crop: Crop,
        /// Use default, off, or the exact microphone name from `microphones`.
        #[arg(long, default_value = "default")]
        microphone: String,
        /// Generated moving pattern for testing the recorder without a camera.
        #[arg(long)]
        test_pattern: bool,
    },
    /// Read the camera's supported vendor commands; does not change settings.
    Probe,
    /// Read selected settings through Canon's remote session.
    Status,
    /// Save one native USB live-view JPEG (not a full-resolution still).
    Preview {
        #[arg(long, default_value = "captures/preview.jpg")]
        output: std::path::PathBuf,
        #[arg(long, default_value = "0", value_parser = rotation)]
        rotate: u16,
    },
    /// Measure actual USB delivery and pixel changes; saves no camera images.
    Benchmark {
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u32).range(2..=36000))]
        frames: u32,
        #[arg(long, default_value = "0", value_parser = rotation)]
        rotate: u16,
    },
    /// Request autofocus for two seconds. Does not claim a focus lock.
    Autofocus,
    /// Move the lens once in live view. Requires a compatible lens in AF mode.
    Focus {
        #[arg(value_enum)]
        direction: Direction,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=3))]
        step: u32,
    },
}
#[derive(Clone, clap::ValueEnum)]
enum Direction {
    Near,
    Far,
}
#[derive(Clone, clap::ValueEnum)]
enum Crop {
    Native,
    Portrait,
    Landscape,
}

fn record(
    output: std::path::PathBuf,
    seconds: u32,
    rotate: u16,
    crop: Crop,
    microphone: String,
    test_pattern: bool,
) -> Result<()> {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    use studio::{Command, Controller, Phase, Settings};
    let running = Arc::new(AtomicBool::new(true));
    let signal = running.clone();
    ctrlc::set_handler(move || signal.store(false, Ordering::Relaxed))?;
    let settings = Settings {
        rotation: rotate,
        aspect: match crop {
            Crop::Native => frame::Aspect::Native,
            Crop::Portrait => frame::Aspect::Portrait,
            Crop::Landscape => frame::Aspect::Landscape,
        },
    };
    let microphone = match microphone.as_str() {
        "off" => audio::Microphone::Off,
        "default" => audio::Microphone::Default,
        _ => audio::Microphone::Named(microphone),
    };
    let controller = Controller::new(test_pattern, settings);
    let result = (|| -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let state = controller.snapshot();
            if let Some(error) = state.error {
                anyhow::bail!("{error}");
            }
            if state.phase == Phase::Preview {
                break;
            }
            anyhow::ensure!(running.load(Ordering::Relaxed), "Cancelled");
            anyhow::ensure!(
                Instant::now() < deadline,
                "Timed out waiting for live preview"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        controller.send(Command::Start(output, microphone))?;
        let mut stop_sent = false;
        let deadline = Instant::now() + Duration::from_secs(u64::from(seconds) + 60);
        loop {
            let state = controller.snapshot();
            if let Some(error) = state.error {
                anyhow::bail!("{error}");
            }
            if let Some(path) = state.last_file {
                println!("Saved {}", path.display());
                return Ok(());
            }
            if state.phase == Phase::Recording
                && !stop_sent
                && (state.elapsed >= f64::from(seconds) || !running.load(Ordering::Relaxed))
            {
                controller.send(Command::Stop)?;
                stop_sent = true;
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "Timed out waiting for the recorder"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
    })();
    controller.shutdown();
    // Allow camera cleanup and MP4 finalization even after an error or Ctrl+C.
    let deadline = Instant::now() + Duration::from_secs(30);
    while controller.snapshot().phase != Phase::Closed && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::ensure!(
        controller.snapshot().phase == Phase::Closed,
        "Camera cleanup is still blocked by Windows; the unfinished recording may need recovery"
    );
    result
}
fn rotation(value: &str) -> Result<u16, String> {
    match value {
        "0" => Ok(0),
        "90" => Ok(90),
        "180" => Ok(180),
        "270" => Ok(270),
        _ => Err("Use 0, 90, 180 or 270 degrees clockwise".into()),
    }
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    let com = wpd::Com::initialize()?;
    match cli.command {
        Command::Studio => {
            drop(com);
            return eos_camera::gui::run();
        }
        Command::Microphones => println!("{}", serde_json::to_string_pretty(&audio::inputs()?)?),
        Command::Record {
            output,
            seconds,
            rotate,
            crop,
            microphone,
            test_pattern,
        } => record(output, seconds, rotate, crop, microphone, test_pattern)?,
        Command::List => println!("{}", serde_json::to_string_pretty(&wpd::devices(&com)?)?),
        Command::Probe => {
            let camera = wpd::Camera::open(&com)?;
            let codes = camera.vendor_opcodes()?;
            let report = serde_json::json!({
                "model": "Canon EOS 600D / Rebel T3i",
                "transport": "Windows Portable Devices (existing USB driver)",
                "vendor_opcodes": codes.iter().map(|c| format!("0x{c:04X}")).collect::<Vec<_>>(),
                "advertises_live_view": codes.contains(&0x9153),
                "advertises_autofocus": codes.contains(&0x9154),
                "advertises_lens_drive": codes.contains(&0x9155),
                "advertises_remote_shutter": codes.contains(&0x9128),
                "note": "Advertised operations require hardware validation; availability can depend on camera mode and lens."
            });
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Status => {
            let camera = wpd::Camera::open(&com)?;
            let session = eos::Session::start(&camera)?;
            println!("{}", serde_json::to_string_pretty(&session.status())?);
        }
        Command::Preview { output, rotate } => {
            use std::io::Write;
            anyhow::ensure!(
                !output.exists(),
                "Output already exists: {}",
                output.display()
            );
            let camera = wpd::Camera::open(&com)?;
            let mut session = eos::Session::start(&camera)?;
            session.start_live_view()?;
            let jpeg = session.frame()?;
            session.stop()?;
            let frame = frame::Frame::decode(&jpeg)?.rotate(rotate)?;
            let jpeg = if rotate == 0 { jpeg } else { frame.jpeg()? };
            if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            // create_new also protects against a file created since the early check.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            file.write_all(&jpeg)?;
            println!(
                "Saved {} × {} live-view frame ({} bytes, {rotate}° clockwise) to {}",
                frame.width,
                frame.height,
                jpeg.len(),
                output.display()
            );
        }
        Command::Benchmark { frames, rotate } => {
            use std::{
                sync::{
                    Arc,
                    atomic::{AtomicBool, Ordering},
                },
                time::Instant,
            };
            let running = Arc::new(AtomicBool::new(true));
            let signal = running.clone();
            ctrlc::set_handler(move || signal.store(false, Ordering::Relaxed))?;
            let camera = wpd::Camera::open(&com)?;
            let mut session = eos::Session::start(&camera)?;
            session.start_live_view()?;
            // Warm up outside the timing interval.
            let mut last = session.frame()?;
            let start = Instant::now();
            let mut keep_alive = Instant::now();
            let mut received = 0;
            let mut changed = 0;
            let mut width = 0;
            let mut height = 0;
            while received < frames && running.load(Ordering::Relaxed) {
                if keep_alive.elapsed().as_secs() >= 5 {
                    session.keep_awake()?;
                    keep_alive = Instant::now();
                }
                let jpeg = session.frame()?;
                let frame = frame::Frame::decode(&jpeg)?.rotate(rotate)?;
                width = frame.width;
                height = frame.height;
                if jpeg != last {
                    changed += 1;
                }
                last = jpeg;
                received += 1;
            }
            let elapsed = start.elapsed().as_secs_f64();
            session.stop()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "frames": received, "changed_jpeg_payloads": changed, "seconds": elapsed,
                    "delivered_fps_including_decode_and_rotation": received as f64 / elapsed,
                    "output_width": width, "output_height": height, "rotation_clockwise": rotate,
                    "interrupted": !running.load(Ordering::Relaxed), "images_saved": false
                }))?
            );
        }
        Command::Autofocus | Command::Focus { .. } => {
            let camera = wpd::Camera::open(&com)?;
            let mut session = eos::Session::start(&camera)?;
            session.start_live_view()?;
            let _ = session.frame()?;
            match cli.command {
                Command::Autofocus => {
                    session.autofocus()?;
                    println!("Autofocus request completed; focus lock has not been verified.");
                }
                Command::Focus { direction, step } => {
                    session.focus_step(matches!(direction, Direction::Far), step)?;
                    println!(
                        "Lens-drive command accepted; optical movement has not been measured."
                    );
                }
                _ => unreachable!(),
            }
            session.stop()?;
        }
    }
    Ok(())
}
