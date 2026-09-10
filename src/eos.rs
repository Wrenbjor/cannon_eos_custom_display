use crate::{
    protocol,
    wpd::{Camera, PtpError},
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    thread,
    time::{Duration, Instant},
};

fn busy(e: &anyhow::Error) -> bool {
    e.downcast_ref::<PtpError>()
        .is_some_and(|p| p.0 == 0x2019 || p.0 == 0xa102)
        || e.downcast_ref::<windows::core::Error>()
            .is_some_and(|w| w.code().0 as u32 == 0x800700aa)
}
fn retry<T>(mut action: impl FnMut() -> Result<T>) -> Result<T> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match action() {
            Err(e) if busy(&e) && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100))
            }
            result => return result,
        }
    }
}

pub struct Session<'a, 'c> {
    camera: &'a Camera<'c>,
    properties: BTreeMap<u32, Vec<u8>>,
    original_output: Option<u32>,
    original_mode: Option<u32>,
}
impl<'a, 'c> Session<'a, 'c> {
    pub fn start(camera: &'a Camera<'c>) -> Result<Self> {
        retry(|| camera.no_data(0x9114, &[1])).context("Enabling Canon remote mode")?;
        let mut s = Self {
            camera,
            properties: BTreeMap::new(),
            original_output: None,
            original_mode: None,
        };
        retry(|| camera.no_data(0x9115, &[1])).context("Enabling Canon events")?;
        s.poll()?;
        for _ in 0..10 {
            if s.properties.contains_key(&0xd1b0) && s.properties.contains_key(&0xd1b1) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
            s.poll()?;
        }
        ensure!(
            !s.properties.is_empty(),
            "Camera returned no properties after remote initialization. Switch it off and on, then retry."
        );
        Ok(s)
    }
    pub fn poll(&mut self) -> Result<()> {
        let data = retry(|| self.camera.read(0x9116, &[], 4 * 1024 * 1024))
            .context("Reading Canon events")?;
        for (kind, body) in protocol::records(&data)? {
            if kind == 0xc189 {
                let code = protocol::word(body, 0)?;
                self.properties.insert(code, body[4..].to_vec());
            }
        }
        Ok(())
    }
    fn numeric(&self, code: u32) -> Result<u32> {
        let bytes = self
            .properties
            .get(&code)
            .with_context(|| format!("Camera did not report property 0x{code:04X}"))?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= 4,
            "Property is not a small integer"
        );
        let mut value = [0; 4];
        value[..bytes.len()].copy_from_slice(bytes);
        Ok(u32::from_le_bytes(value))
    }
    fn set_numeric(&self, code: u32, value: u32) -> Result<()> {
        retry(|| {
            self.camera
                .write(0x9110, &[], &protocol::property_packet(code, value))
        })
        .with_context(|| format!("Setting Canon property 0x{code:04X}"))
    }
    pub fn status(&self) -> serde_json::Value {
        // Explicit allowlist: never print owner names, copyright fields or serials.
        let fields = [
            ("aperture_code", 0xd101),
            ("shutter_code", 0xd102),
            ("iso_code", 0xd103),
            ("exposure_mode_code", 0xd105),
            ("focus_mode_code", 0xd108),
            ("battery_code", 0xd111),
            ("evf_output", 0xd1b0),
            ("evf_mode", 0xd1b1),
        ];
        let mut report = serde_json::Map::new();
        for (name, code) in fields {
            if let Ok(value) = self.numeric(code) {
                report.insert(name.into(), value.into());
            } else if let Some(bytes) = self.properties.get(&code) {
                report.insert(name.into(), serde_json::json!({"bytes": bytes}));
            }
        }
        report.into()
    }
    pub fn start_live_view(&mut self) -> Result<()> {
        self.poll()?;
        let output = self.numeric(0xd1b0)?;
        let mode = self.numeric(0xd1b1)?;
        if mode != 1 {
            self.original_mode = Some(mode);
            if let Err(e) = self.set_numeric(0xd1b1, 1) {
                if !busy(&e) {
                    return Err(e);
                }
                self.original_mode = None;
            }
        }
        if output & 2 == 0 {
            self.original_output = Some(output);
            self.set_numeric(0xd1b0, output | 2)?;
        }
        self.camera.no_data(0x911d, &[])?;
        Ok(())
    }
    pub fn frame(&mut self) -> Result<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            self.poll()?;
            match self
                .camera
                .read(0x9153, &[0x0020_0000, 0, 0], 16 * 1024 * 1024)
            {
                Ok(data) => return Ok(protocol::preview_jpeg(&data)?.to_vec()),
                Err(e) if busy(&e) && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50))
                }
                Err(e) => return Err(e.context("Retrieving live-view frame")),
            }
        }
    }
    pub fn keep_awake(&self) -> Result<()> {
        retry(|| self.camera.no_data(0x911d, &[]))
    }
    pub fn focus_step(&self, far: bool, step: u32) -> Result<()> {
        ensure!((1..=3).contains(&step), "Focus step must be 1, 2 or 3");
        // Lens movement is not retried: a lost reply must not cause double motion.
        self.camera
            .no_data(0x9155, &[step | if far { 0x8000 } else { 0 }])
    }
    pub fn autofocus(&mut self) -> Result<()> {
        let result = (|| -> Result<()> {
            self.camera.no_data(0x9154, &[])?;
            let until = Instant::now() + Duration::from_secs(2);
            while Instant::now() < until {
                self.poll()?;
                thread::sleep(Duration::from_millis(100));
            }
            Ok(())
        })();
        let cancelled = self.camera.no_data(0x9160, &[]);
        result?;
        cancelled
    }
    pub fn stop(&mut self) -> Result<()> {
        let output_result = self
            .original_output
            .take()
            .map(|v| self.set_numeric(0xd1b0, v))
            .transpose();
        let mode_result = self
            .original_mode
            .take()
            .map(|v| self.set_numeric(0xd1b1, v))
            .transpose();
        output_result?;
        mode_result?;
        Ok(())
    }
}
impl Drop for Session<'_, '_> {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            eprintln!(
                "Could not restore live view: {e:#}. Switch the camera off and on if necessary."
            );
        }
        let _ = self.camera.no_data(0x9115, &[0]);
        let _ = self.camera.no_data(0x9114, &[0]);
    }
}
