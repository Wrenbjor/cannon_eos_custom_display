//! Windows owns the PTP session. All Canon requests use WPD's MTP extension,
//! preserving the camera's existing USB driver.
use anyhow::{Context, Result, ensure};
use std::{marker::PhantomData, rc::Rc};
use windows::{
    Win32::{
        Devices::PortableDevices::*,
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, PROPERTYKEY},
        System::Com::{StructuredStorage::PROPVARIANT, *},
        System::Threading::CreateMutexW,
    },
    core::{HSTRING, PWSTR, w},
};

pub struct Com(PhantomData<Rc<()>>);
impl Com {
    pub fn initialize() -> Result<Self> {
        // This guard and all devices stay on the initializing thread.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        Ok(Self(PhantomData))
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}

#[derive(serde::Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub supported_model: bool,
    #[serde(skip)]
    id: String,
}

unsafe fn take_string(raw: PWSTR) -> Result<String> {
    let result = unsafe { raw.to_string() };
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    Ok(result?)
}

pub fn devices(_com: &Com) -> Result<Vec<DeviceInfo>> {
    unsafe {
        let manager: IPortableDeviceManager =
            CoCreateInstance(&PortableDeviceManager, None, CLSCTX_INPROC_SERVER)?;
        let mut count = 0;
        manager.GetDevices(std::ptr::null_mut(), &mut count)?;
        let mut ids = vec![PWSTR::null(); count as usize];
        if count == 0 {
            return Ok(vec![]);
        }
        manager.GetDevices(ids.as_mut_ptr(), &mut count)?;
        let mut result = Vec::new();
        for id in ids {
            if id.is_null() {
                continue;
            }
            let owned_id = take_string(id)?;
            let mut name_size = 0;
            let wide_id = HSTRING::from(&owned_id);
            manager.GetDeviceFriendlyName(&wide_id, PWSTR::null(), &mut name_size)?;
            let mut name = vec![0u16; name_size as usize];
            manager.GetDeviceFriendlyName(&wide_id, PWSTR(name.as_mut_ptr()), &mut name_size)?;
            let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
            result.push(DeviceInfo {
                name: String::from_utf16_lossy(&name[..end]),
                supported_model: owned_id.to_ascii_uppercase().contains("VID_04A9&PID_3218"),
                id: owned_id,
            });
        }
        Ok(result)
    }
}

pub struct Camera<'a> {
    device: IPortableDevice,
    _com: &'a Com,
    _lease: Lease,
}

struct Lease(HANDLE);
impl Drop for Lease {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
impl Lease {
    fn acquire() -> Result<Self> {
        unsafe {
            let handle = CreateMutexW(None, false, w!("Local\\OpenEosCamera.04A9.3218"))?;
            let exists = GetLastError() == ERROR_ALREADY_EXISTS;
            let lease = Self(handle);
            ensure!(
                !exists,
                "Another Open EOS Camera process already owns the camera connection"
            );
            Ok(lease)
        }
    }
}

fn values() -> Result<IPortableDeviceValues> {
    Ok(unsafe { CoCreateInstance(&PortableDeviceValues, None, CLSCTX_INPROC_SERVER)? })
}
fn command(key: &PROPERTYKEY) -> Result<IPortableDeviceValues> {
    let v = values()?;
    unsafe {
        v.SetGuidValue(&WPD_PROPERTY_COMMON_COMMAND_CATEGORY, &key.fmtid)?;
        v.SetUnsignedIntegerValue(&WPD_PROPERTY_COMMON_COMMAND_ID, key.pid)?;
    }
    Ok(v)
}
fn operation(key: &PROPERTYKEY, opcode: u16, params: &[u32]) -> Result<IPortableDeviceValues> {
    ensure!(params.len() <= 5, "PTP accepts at most five parameters");
    let v = command(key)?;
    unsafe {
        let p: IPortableDevicePropVariantCollection = CoCreateInstance(
            &PortableDevicePropVariantCollection,
            None,
            CLSCTX_INPROC_SERVER,
        )?;
        for value in params {
            p.Add(&PROPVARIANT::from(*value))?;
        }
        v.SetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_OPERATION_CODE, opcode as u32)?;
        v.SetIPortableDevicePropVariantCollectionValue(&WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS, &p)?;
    }
    Ok(v)
}

#[derive(Debug)]
pub struct PtpError(pub u32);
impl std::fmt::Display for PtpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "camera returned PTP 0x{:04X}", self.0)
    }
}
impl std::error::Error for PtpError {}
fn check_response(v: &IPortableDeviceValues) -> Result<()> {
    let code = unsafe { v.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_RESPONSE_CODE)? };
    if code != 0x2001 {
        return Err(PtpError(code).into());
    }
    Ok(())
}

impl<'a> Camera<'a> {
    pub fn open(com: &'a Com) -> Result<Self> {
        let lease = Lease::acquire()?;
        let mut matching = devices(com)?.into_iter().filter(|d| d.supported_model);
        let info = matching
            .next()
            .context("No EOS 600D / Rebel T3i found. Connect USB and switch the camera on.")?;
        ensure!(
            matching.next().is_none(),
            "Multiple T3i cameras found; connect just one for this prototype"
        );
        unsafe {
            let device: IPortableDevice =
                CoCreateInstance(&PortableDeviceFTM, None, CLSCTX_INPROC_SERVER)?;
            let client = values()?;
            client.SetStringValue(&WPD_CLIENT_NAME, &HSTRING::from("Open EOS Camera"))?;
            client.SetUnsignedIntegerValue(&WPD_CLIENT_MAJOR_VERSION, 0)?;
            client.SetUnsignedIntegerValue(&WPD_CLIENT_MINOR_VERSION, 1)?;
            device.Open(&HSTRING::from(info.id), &client).context("Could not open camera. Close Canon EOS Utility and other camera software, then retry.")?;
            Ok(Self {
                device,
                _com: com,
                _lease: lease,
            })
        }
    }
    fn send(&self, request: &IPortableDeviceValues) -> Result<IPortableDeviceValues> {
        unsafe {
            let result = self.device.SendCommand(0, request)?;
            result
                .GetErrorValue(&WPD_PROPERTY_COMMON_HRESULT)?
                .ok()
                .context("Windows WPD command failed")?;
            Ok(result)
        }
    }
    pub fn vendor_opcodes(&self) -> Result<Vec<u32>> {
        let v = self.send(&command(&WPD_COMMAND_MTP_EXT_GET_SUPPORTED_VENDOR_OPCODES)?)?;
        unsafe {
            let codes = v.GetIPortableDevicePropVariantCollectionValue(
                &WPD_PROPERTY_MTP_EXT_VENDOR_OPERATION_CODES,
            )?;
            let mut count = 0;
            // Windows metadata marks these output pointers const. Preserve a
            // mutable allocation and pass raw pointers; COM writes into them.
            codes.GetCount(&raw mut count)?;
            ensure!(count <= 65536, "Implausible opcode count");
            let mut result = Vec::new();
            for i in 0..count {
                let mut p = PROPVARIANT::default();
                codes.GetAt(i, &raw mut p)?;
                result.push(u32::try_from(&p)?);
            }
            Ok(result)
        }
    }
    pub fn no_data(&self, opcode: u16, params: &[u32]) -> Result<()> {
        check_response(&self.send(&operation(
            &WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITHOUT_DATA_PHASE,
            opcode,
            params,
        )?)?)
    }
    fn end(&self, context: &str) -> Result<()> {
        let v = command(&WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER)?;
        unsafe {
            v.SetStringValue(
                &WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT,
                &HSTRING::from(context),
            )?;
        }
        check_response(&self.send(&v)?)
    }
    pub fn read(&self, opcode: u16, params: &[u32], limit: usize) -> Result<Vec<u8>> {
        let v = self.send(&operation(
            &WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ,
            opcode,
            params,
        )?)?;
        // A response without a context is a camera-side rejection, not a transfer.
        let context = match unsafe { v.GetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT) } {
            Ok(s) => unsafe { take_string(s)? },
            Err(e) => {
                check_response(&v)?;
                return Err(e.into());
            }
        };
        let result = (|| -> Result<Vec<u8>> {
            let total = unsafe {
                v.GetUnsignedLargeIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE)?
            };
            ensure!(
                total <= limit as u64,
                "Transfer is {total} bytes, exceeding the {limit}-byte limit"
            );
            let chunk_size = unsafe {
                v.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE)
            }
            .unwrap_or(64 * 1024)
            .clamp(1, 1024 * 1024);
            let mut data = Vec::with_capacity(total as usize);
            while data.len() < total as usize {
                let request = command(&WPD_COMMAND_MTP_EXT_READ_DATA)?;
                let wanted = chunk_size.min((total as usize - data.len()) as u32);
                unsafe {
                    request.SetStringValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT,
                        &HSTRING::from(&context),
                    )?;
                    request.SetUnsignedIntegerValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ,
                        wanted,
                    )?;
                    request.SetBufferValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_DATA,
                        &vec![0; wanted as usize],
                    )?;
                }
                let response = self.send(&request)?;
                let mut raw = std::ptr::null_mut();
                let mut length = 0;
                unsafe {
                    response.GetBufferValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_DATA,
                        &mut raw,
                        &mut length,
                    )?;
                }
                let reported = unsafe {
                    response.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ)
                };
                let valid = !raw.is_null()
                    && length > 0
                    && length <= wanted
                    && reported.as_ref().is_ok_and(|n| *n == length);
                if valid {
                    data.extend_from_slice(unsafe {
                        std::slice::from_raw_parts(raw, length as usize)
                    });
                }
                unsafe {
                    CoTaskMemFree(Some(raw.cast()));
                }
                ensure!(valid, "Invalid or stalled WPD transfer");
            }
            Ok(data)
        })();
        let ended = self.end(&context);
        let data = result?;
        ended?;
        Ok(data)
    }
    pub fn write(&self, opcode: u16, params: &[u32], data: &[u8]) -> Result<()> {
        let request = operation(
            &WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_WRITE,
            opcode,
            params,
        )?;
        unsafe {
            request.SetUnsignedLargeIntegerValue(
                &WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE,
                data.len() as u64,
            )?;
        }
        let response = self.send(&request)?;
        let context =
            match unsafe { response.GetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT) } {
                Ok(s) => unsafe { take_string(s)? },
                Err(e) => {
                    check_response(&response)?;
                    return Err(e.into());
                }
            };
        let result = (|| -> Result<()> {
            for chunk in data.chunks(64 * 1024) {
                let v = command(&WPD_COMMAND_MTP_EXT_WRITE_DATA)?;
                unsafe {
                    v.SetStringValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT,
                        &HSTRING::from(&context),
                    )?;
                    v.SetUnsignedIntegerValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_WRITE,
                        chunk.len() as u32,
                    )?;
                    v.SetBufferValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, chunk)?;
                }
                let r = self.send(&v)?;
                let written = unsafe {
                    r.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_WRITTEN)?
                };
                ensure!(written as usize == chunk.len(), "Short WPD write");
            }
            Ok(())
        })();
        let ended = self.end(&context);
        result?;
        ended
    }
}
impl Drop for Camera<'_> {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.Close();
        }
    }
}
