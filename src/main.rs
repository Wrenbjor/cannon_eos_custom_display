mod eos;
mod frame;
mod protocol;
mod wpd;

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
