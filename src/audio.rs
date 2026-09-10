use anyhow::{Context, Result, bail, ensure};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    sync::{
        Arc, Mutex, OnceLock,
        mpsc::{self, Receiver, SyncSender},
    },
    time::Instant,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Microphone {
    Off,
    Default,
    Named(String),
}
#[derive(Clone, Copy, Debug)]
pub struct AudioFormat {
    pub rate: u32,
    pub channels: u16,
}
pub struct AudioBlock {
    pub time: i64,
    pub samples: Vec<i16>,
}
pub struct Capture {
    pub format: AudioFormat,
    pub blocks: Receiver<AudioBlock>,
    pub failure: Arc<Mutex<Option<String>>>,
    epoch: Arc<OnceLock<Instant>>,
    stream: Option<cpal::Stream>,
}

pub fn inputs() -> Result<Vec<String>> {
    let mut names = Vec::new();
    for device in cpal::default_host().input_devices()? {
        names.push(device.name()?);
    }
    names.sort();
    names.dedup();
    Ok(names)
}
fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: SyncSender<AudioBlock>,
    failure: Arc<Mutex<Option<String>>>,
    epoch: Arc<OnceLock<Instant>>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    i16: cpal::FromSample<T>,
{
    let (rate, channels) = (config.sample_rate.0, u64::from(config.channels));
    let mut frames = 0u64;
    let mut anchor = None;
    let callback_failure = failure.clone();
    Ok(device.build_input_stream(config,
        move |data: &[T], info: &cpal::InputCallbackInfo| {
            let Some(epoch) = epoch.get() else { return; };
            if data.is_empty() { return; }
            let times = info.timestamp();
            let latency = times.callback.duration_since(&times.capture).unwrap_or_default();
            let now = Instant::now();
            let captured = now.checked_sub(latency).unwrap_or(now);
            let offset = *anchor.get_or_insert_with(|| (captured.saturating_duration_since(*epoch).as_nanos() / 100) as i64);
            let time = offset + sample_time(frames, rate);
            let samples: Vec<i16> = data.iter().map(|sample| sample.to_sample::<i16>()).collect();
            frames += samples.len() as u64 / channels;
            if tx.try_send(AudioBlock { time, samples }).is_err()
                && let Ok(mut message) = callback_failure.lock() { *message = Some("Microphone buffering overflowed; recording was stopped to avoid silently losing audio.".into()); }
        },
        move |error| { if let Ok(mut message) = failure.lock() { *message = Some(format!("Microphone disconnected or failed: {error}")); } }, None)? )
}
pub fn sample_time(frames: u64, rate: u32) -> i64 {
    ((u128::from(frames) * 10_000_000) / u128::from(rate)) as i64
}

impl Capture {
    pub fn open(selection: &Microphone) -> Result<Self> {
        let host = cpal::default_host();
        let device = match selection {
            Microphone::Default => host
                .default_input_device()
                .context("No default Windows microphone was found")?,
            Microphone::Named(name) => {
                let mut matches = host
                    .input_devices()?
                    .filter(|d| d.name().is_ok_and(|n| n == *name));
                let device = matches
                    .next()
                    .context("Selected microphone is no longer connected")?;
                ensure!(
                    matches.next().is_none(),
                    "More than one microphone has this name; choose Windows default instead"
                );
                device
            }
            Microphone::Off => bail!("Audio is disabled"),
        };
        let config = device
            .default_input_config()
            .context("Reading microphone format")?;
        let format = AudioFormat {
            rate: config.sample_rate().0,
            channels: config.channels(),
        };
        ensure!(
            [44100, 48000].contains(&format.rate) && [1, 2].contains(&format.channels),
            "Microphone uses {} Hz / {} channels. Set its Windows format to 44.1 or 48 kHz, mono or stereo.",
            format.rate,
            format.channels
        );
        // Each callback is bounded by the device buffer; never block its audio thread.
        let (tx, blocks) = mpsc::sync_channel(512);
        let failure = Arc::new(Mutex::new(None));
        let epoch = Arc::new(OnceLock::new());
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(
                &device,
                &config.config(),
                tx,
                failure.clone(),
                epoch.clone(),
            )?,
            cpal::SampleFormat::I16 => build::<i16>(
                &device,
                &config.config(),
                tx,
                failure.clone(),
                epoch.clone(),
            )?,
            cpal::SampleFormat::U16 => build::<u16>(
                &device,
                &config.config(),
                tx,
                failure.clone(),
                epoch.clone(),
            )?,
            other => bail!("Microphone sample format {other} is not supported yet"),
        };
        Ok(Self {
            format,
            blocks,
            failure,
            epoch,
            stream: Some(stream),
        })
    }
    pub fn start(&self, epoch: Instant) -> Result<()> {
        self.epoch
            .set(epoch)
            .map_err(|_| anyhow::anyhow!("Microphone is already started"))?;
        self.stream
            .as_ref()
            .context("Microphone is closed")?
            .play()?;
        Ok(())
    }
    pub fn stop(&mut self) {
        self.stream.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_clock_does_not_accumulate_rounding_error() {
        for rate in [44100, 48000] {
            assert_eq!(sample_time(u64::from(rate) * 3600, rate), 36_000_000_000);
            assert!(sample_time(1025, rate) > sample_time(1024, rate));
        }
    }
}
