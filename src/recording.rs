//! MP4 recording through the operating system. No FFmpeg process is required.
use crate::{
    audio::{self, AudioFormat, Capture, Microphone},
    frame::Frame,
};
use anyhow::{Context, Result, ensure};
use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{Media::MediaFoundation::*, Storage::FileSystem::MoveFileW},
    core::{GUID, HSTRING, PCWSTR},
};

pub struct Runtime(PhantomData<Rc<()>>);
impl Runtime {
    pub fn start() -> Result<Self> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        }
        Ok(Self(PhantomData))
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

pub struct Mp4 {
    writer: Option<IMFSinkWriter>,
    stream: Option<IMFByteStream>,
    video_index: u32,
    audio_index: Option<u32>,
    audio_format: Option<AudioFormat>,
    width: u16,
    height: u16,
    final_path: PathBuf,
    partial_path: PathBuf,
    video_end: i64,
    audio_end: i64,
    pub frames: u64,
    pub audio_frames: u64,
}

fn video_type(subtype: &GUID, width: u16, height: u16) -> Result<IMFMediaType> {
    unsafe {
        let kind = MFCreateMediaType()?;
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        kind.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        kind.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        kind.SetUINT64(
            &MF_MT_FRAME_SIZE,
            (u64::from(width) << 32) | u64::from(height),
        )?;
        kind.SetUINT64(&MF_MT_FRAME_RATE, (30u64 << 32) | 1)?;
        kind.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        kind.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
        kind.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
        kind.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32)?;
        kind.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
        Ok(kind)
    }
}
fn audio_type(format: AudioFormat, compressed: bool) -> Result<IMFMediaType> {
    unsafe {
        let kind = MFCreateMediaType()?;
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        kind.SetGUID(
            &MF_MT_SUBTYPE,
            if compressed {
                &MFAudioFormat_AAC
            } else {
                &MFAudioFormat_PCM
            },
        )?;
        kind.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(format.channels))?;
        kind.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, format.rate)?;
        kind.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        if compressed {
            kind.SetUINT32(
                &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
                if format.channels == 1 { 16000 } else { 24000 },
            )?;
            kind.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
        } else {
            kind.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, u32::from(format.channels) * 2)?;
            kind.SetUINT32(
                &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
                format.rate * u32::from(format.channels) * 2,
            )?;
            kind.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        }
        Ok(kind)
    }
}

impl Mp4 {
    pub fn create(
        path: &Path,
        width: u16,
        height: u16,
        audio: Option<AudioFormat>,
    ) -> Result<Self> {
        ensure!(
            width >= 2 && height >= 2 && width.is_multiple_of(2) && height.is_multiple_of(2),
            "Video dimensions must be positive and even"
        );
        ensure!(
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mp4")),
            "Use an .mp4 filename"
        );
        ensure!(
            !path.exists(),
            "Recording already exists: {}",
            path.display()
        );
        let final_path = std::path::absolute(path)?;
        let parent = final_path
            .parent()
            .context("Recording has no output folder")?;
        std::fs::create_dir_all(parent).context("Creating recording folder")?;
        let partial_path = final_path.with_extension("recording.mp4");
        unsafe {
            // Windows creates this exclusively. Never truncate an existing recording.
            let stream = MFCreateFile(
                MF_ACCESSMODE_WRITE,
                MF_OPENMODE_FAIL_IF_EXIST,
                MF_FILEFLAGS_NONE,
                &HSTRING::from(partial_path.as_os_str()),
            )
            .context(
                "Creating recording file (an existing or unfinished file will not be overwritten)",
            )?;
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 2)?;
            let attributes = attributes.context("No media attributes")?;
            attributes.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)?;
            // Use the portable software path first; hardware encoders vary by GPU.
            let writer = MFCreateSinkWriterFromURL(PCWSTR::null(), &stream, &attributes)
                .context("Creating MP4 writer")?;
            let output = video_type(&MFVideoFormat_H264, width, height)?;
            output.SetUINT32(&MF_MT_AVG_BITRATE, 8_000_000)?;
            let video_index = writer.AddStream(&output)?;
            writer
                .SetInputMediaType(
                    video_index,
                    &video_type(&MFVideoFormat_NV12, width, height)?,
                    None::<&IMFAttributes>,
                )
                .context("Starting the Windows H.264 encoder")?;
            let audio_index = if let Some(format) = audio {
                let index = writer.AddStream(&audio_type(format, true)?)?;
                writer
                    .SetInputMediaType(index, &audio_type(format, false)?, None::<&IMFAttributes>)
                    .context("Starting the Windows AAC encoder")?;
                Some(index)
            } else {
                None
            };
            writer.BeginWriting()?;
            Ok(Self {
                writer: Some(writer),
                stream: Some(stream),
                video_index,
                audio_index,
                audio_format: audio,
                width,
                height,
                final_path,
                partial_path,
                video_end: 0,
                audio_end: 0,
                frames: 0,
                audio_frames: 0,
            })
        }
    }
    fn sample(&self, index: u32, data: &[u8], time: i64, duration: i64) -> Result<()> {
        ensure!(time >= 0 && duration > 0, "Invalid media timestamp");
        unsafe {
            let buffer = MFCreateMemoryBuffer(u32::try_from(data.len())?)?;
            let mut bytes = std::ptr::null_mut();
            buffer.Lock(&mut bytes, None, None)?;
            std::ptr::copy_nonoverlapping(data.as_ptr(), bytes, data.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(data.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(duration)?;
            self.writer
                .as_ref()
                .context("Recording is closed")?
                .WriteSample(index, &sample)?;
        }
        Ok(())
    }
    pub fn video(&mut self, frame: &Frame, time: i64, duration: i64) -> Result<()> {
        ensure!(
            frame.width == self.width && frame.height == self.height,
            "Recording dimensions changed"
        );
        ensure!(
            time >= self.video_end,
            "Video timestamps moved backwards or overlap"
        );
        self.sample(self.video_index, &frame.nv12()?, time, duration)?;
        self.video_end = time + duration;
        self.frames += 1;
        Ok(())
    }
    pub fn audio(&mut self, samples: &[i16], time: i64) -> Result<()> {
        let format = self.audio_format.context("No audio stream")?;
        ensure!(
            !samples.is_empty() && samples.len().is_multiple_of(usize::from(format.channels)),
            "Invalid PCM audio block"
        );
        ensure!(
            time + 1 >= self.audio_end,
            "Audio timestamps moved backwards"
        );
        let frames = samples.len() as u64 / u64::from(format.channels);
        let duration = audio::sample_time(frames, format.rate);
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.sample(
            self.audio_index.context("No AAC stream")?,
            &bytes,
            time,
            duration,
        )?;
        self.audio_frames += frames;
        self.audio_end = time + duration;
        Ok(())
    }
    pub fn finish(&mut self) -> Result<PathBuf> {
        ensure!(
            self.frames > 0,
            "No video frames were recorded; unfinished file retained at {}",
            self.partial_path.display()
        );
        let writer = self.writer.take().context("Recording already finalized")?;
        let result = unsafe { writer.Finalize() };
        // Releasing the writer shuts down its media sink and closes the byte
        // stream. Calling Close again after that can return E_INVALIDARG.
        drop(writer);
        drop(self.stream.take());
        result.with_context(|| {
            format!(
                "Finalizing MP4; unfinished file retained at {}",
                self.partial_path.display()
            )
        })?;
        // MoveFileW fails if destination exists; std::fs::rename can replace it.
        unsafe {
            MoveFileW(
                &HSTRING::from(self.partial_path.as_os_str()),
                &HSTRING::from(self.final_path.as_os_str()),
            )?;
        }
        Ok(self.final_path.clone())
    }
}
impl Drop for Mp4 {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            unsafe {
                let _ = writer.Finalize();
            }
        }
        if let Some(stream) = self.stream.take() {
            unsafe {
                let _ = stream.Close();
            }
        }
    }
}

pub struct Recording {
    sink: Mp4,
    microphone: Option<Capture>,
    pending: Option<(Arc<Frame>, i64)>,
    pub epoch: Instant,
    pub peak: f32,
    _runtime: Runtime,
}
impl Recording {
    pub fn start(path: &Path, first: Arc<Frame>, mic: &Microphone) -> Result<Self> {
        let runtime = Runtime::start()?;
        let microphone = if *mic == Microphone::Off {
            None
        } else {
            Some(Capture::open(mic)?)
        };
        let sink = Mp4::create(
            path,
            first.width,
            first.height,
            microphone.as_ref().map(|m| m.format),
        )?;
        let epoch = Instant::now();
        if let Some(m) = &microphone {
            m.start(epoch)?;
        }
        Ok(Self {
            sink,
            microphone,
            pending: Some((first, 0)),
            epoch,
            peak: 0.0,
            _runtime: runtime,
        })
    }
    pub fn drain_audio(&mut self) -> Result<()> {
        if let Some(m) = &self.microphone {
            self.peak *= 0.85;
            for block in m.blocks.try_iter() {
                self.peak = self.peak.max(
                    block
                        .samples
                        .iter()
                        .map(|s| f32::from(*s).abs() / 32768.0)
                        .fold(0.0, f32::max),
                );
                self.sink.audio(&block.samples, block.time)?;
            }
            if let Some(error) = m.failure.lock().unwrap().as_ref() {
                anyhow::bail!("{error}");
            }
        }
        Ok(())
    }
    pub fn push(&mut self, frame: Arc<Frame>, captured: Instant) -> Result<()> {
        self.drain_audio()?;
        let time = ticks(captured.saturating_duration_since(self.epoch));
        if let Some((last, last_time)) = self.pending.take() {
            let next = time.max(last_time + 1);
            self.sink.video(&last, last_time, next - last_time)?;
            self.pending = Some((frame, next));
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<PathBuf> {
        if let Some(m) = &mut self.microphone {
            m.stop();
        }
        // Drain buffered audio even if a device error caused this stop.
        if let Some(m) = &self.microphone {
            for block in m.blocks.try_iter() {
                self.sink.audio(&block.samples, block.time)?;
            }
        }
        if let Some((last, time)) = self.pending.take() {
            let end = ticks(self.epoch.elapsed()).max(time + 1);
            self.sink.video(&last, time, end - time)?;
        }
        if self.microphone.is_some() {
            ensure!(
                self.sink.audio_frames > 0,
                "Microphone delivered no audio; unfinished recording was retained"
            );
        }
        self.sink.finish()
    }
}
pub fn ticks(duration: Duration) -> i64 {
    (duration.as_nanos() / 100).min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoder_writes_both_streams_and_never_overwrites() -> Result<()> {
        let _com = crate::wpd::Com::initialize()?;
        let _mf = Runtime::start()?;
        let path = std::env::temp_dir().join(format!(
            "open-eos-test-{}-{}.mp4",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let mut sink = Mp4::create(
            &path,
            64,
            96,
            Some(AudioFormat {
                rate: 48000,
                channels: 1,
            }),
        )?;
        for index in 0..10 {
            sink.audio(&vec![1000i16; 4800], index * 1_000_000)?;
            sink.video(
                &Frame {
                    width: 64,
                    height: 96,
                    rgb: vec![(index * 20) as u8; 64 * 96 * 3],
                },
                index * 1_000_000,
                1_000_000,
            )?;
        }
        sink.finish()?;
        let bytes = std::fs::read(&path)?;
        ensure!(
            bytes.windows(4).any(|s| s == b"moov") && bytes.windows(4).any(|s| s == b"mdat"),
            "MP4 was not finalized"
        );
        ensure!(
            bytes.windows(4).any(|s| s == b"avc1") && bytes.windows(4).any(|s| s == b"mp4a"),
            "Missing H.264 or AAC stream"
        );
        assert!(Mp4::create(&path, 64, 96, None).is_err());
        assert_eq!(std::fs::read(&path)?, bytes);
        std::fs::remove_file(path)?;
        Ok(())
    }
}
