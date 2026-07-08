//! DXGI Desktop Duplication API capture.
//! This is the primary capture path on Windows 8+ systems.

use anyhow::{Context, Result};
use log::{error, info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::DesktopDuplication::*;

use crate::model::{Frame, MonitorInfo, Region};

/// DXGI-based screen capturer using Desktop Duplication API.
pub struct DxgiCapturer {
    device: Option<ID3D11Device>,
    context: Option<ID3D11DeviceContext>,
    duplication: Option<IDXGIOutputDuplication>,
    monitor_info: Option<MonitorInfo>,
    frame_index: u64,
}

impl DxgiCapturer {
    pub fn new() -> Self {
        Self {
            device: None,
            context: None,
            duplication: None,
            monitor_info: None,
            frame_index: 0,
        }
    }

    /// Initialize the capturer for a specific monitor (by adapter and output index).
    pub fn initialize(&mut self, monitor: &MonitorInfo) -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .unwrap_or_default();
        }

        // Create D3D11 device
        let device = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_FLAGS::default(),
                None,
                0,
                D3D11_SDK_VERSION,
            )
            .context("Failed to create D3D11 device for DXGI capture")?
        };
        let device: ID3D11Device = device;
        let context = unsafe { device.GetImmediateContext() }.context("Failed to get immediate context")?;

        // Get DXGI device
        let dxgi_device: IDXGIDevice = device.cast().context("Failed to get DXGI device")?;
        let adapter = unsafe { dxgi_device.GetAdapter() }.context("Failed to get adapter")?;

        // Get the output (monitor) by index or by matching region
        let output = find_output(&adapter, monitor)?;

        // Create duplication interface
        let duplication = unsafe {
            output.DuplicateOutput(&device as *const ID3D11Device as *const IUnknown)
        };

        let duplication = match duplication {
            Ok(d) => d,
            Err(e) => {
                return Err(anyhow::anyhow!("DuplicateOutput failed (DXGI). Is this monitor already duplicated? Error: {:?}", e));
            }
        };

        self.device = Some(device);
        self.context = Some(context);
        self.duplication = Some(duplication);
        self.monitor_info = Some(monitor.clone());
        self.frame_index = 0;

        info!("DXGI capturer initialized for monitor {}", monitor.output_name.trim());
        Ok(())
    }

    /// Capture a single frame. Blocks until a new frame is available or timeout.
    /// Returns Some(Frame) on new frame, None on timeout.
    pub fn capture_frame(&mut self, timeout_ms: u32) -> Result<Option<Frame>> {
        let duplication = self.duplication.as_ref()
            .context("DXGI capturer not initialized")?;
        let device = self.device.as_ref()
            .context("D3D11 device not initialized")?;
        let context = self.context.as_ref()
            .context("D3D11 context not initialized")?;
        let monitor = self.monitor_info.as_ref()
            .context("No monitor info")?;

        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut desktop_resource: Option<IDXGIResource> = None;

        let timeout = std::time::Duration::from_millis(timeout_ms as u64);

        let result = unsafe {
            duplication.AcquireNextFrame(
                timeout_ms,
                &mut frame_info,
                &mut desktop_resource,
            )
        };

        match result {
            Ok(()) => {}
            Err(e) => {
                // DXGI_ERROR_WAIT_TIMEOUT means no new frame
                if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                    return Ok(None);
                }
                // DXGI_ERROR_ACCESS_LOST means desktop duplication was lost (lock screen, RDP disconnect, etc.)
                if e.code() == DXGI_ERROR_ACCESS_LOST {
                    return Err(anyhow::anyhow!("DXGI access lost (screen locked or session disconnected)"));
                }
                return Err(anyhow::anyhow!("DXGI AcquireNextFrame failed: {:?}", e));
            }
        }

        // Get the actual texture
        let resource = desktop_resource.context("No desktop resource from AcquireNextFrame")?;

        let texture: ID3D11Texture2D = unsafe { resource.cast() }
            .context("Failed to cast resource to D3D11 texture")?;

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };

        // Create a staging texture for CPU readback
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: desc.Width,
            Height: desc.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: D3D11_BIND_FLAG(0),
            CPUAccessFlags: D3D11_CPU_ACCESS_READ,
            MiscFlags: D3D11_RESOURCE_MISC_FLAG(0),
        };

        let staging_texture = unsafe {
            device.CreateTexture2D(&staging_desc, None)
                .context("Failed to create staging texture")?
        };

        let src_box = windows::Win32::Graphics::Direct3D11::D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: desc.Width,
            bottom: desc.Height,
            back: 1,
        };

        unsafe {
            context.CopySubresourceRegion(
                &staging_texture,
                0,
                0,
                0,
                0,
                &texture,
                0,
                Some(&src_box),
            );
        }

        // Map the staging texture for reading
        let mapped = unsafe {
            context.Map(
                &staging_texture,
                0,
                D3D11_MAP_READ,
                0,
            ).context("Failed to map staging texture")?
        };

        let stride = mapped.RowPitch;
        let data_size = (stride * desc.Height) as usize;
        let src_slice = std::slice::from_raw_parts(mapped.pData as *const u8, data_size);

        let mut data = Vec::with_capacity(data_size);
        data.extend_from_slice(src_slice);

        unsafe {
            context.Unmap(&staging_texture, 0);
        }

        // Release the frame
        if unsafe { duplication.ReleaseFrame().is_err() } {
            warn!("Failed to release DXGI frame (this is usually non-fatal)");
        }

        self.frame_index += 1;

        let frame = Frame::new(
            data,
            desc.Width,
            desc.Height,
            stride,
            self.frame_index,
            current_timestamp_ms(),
        );

        Ok(Some(frame))
    }

    /// Release the duplication interface (useful when pausing).
    pub fn release(&mut self) {
        self.duplication = None;
        self.context = None;
        self.device = None;
    }

    pub fn is_initialized(&self) -> bool {
        self.duplication.is_some()
    }
}

/// Find the specific DXGI output matching our monitor info.
fn find_output(adapter: &IDXGIAdapter, monitor: &MonitorInfo) -> Result<IDXGIOutput> {
    let mut output_index: u32 = 0;
    loop {
        let output = unsafe { adapter.EnumOutputs(output_index) };
        match output {
            Ok(o) => {
                let desc = unsafe { o.GetDesc() }.unwrap_or_default();
                let rc = desc.DesktopCoordinates;

                // Match by region
                if rc.left == monitor.region.x
                    && rc.top == monitor.region.y
                    && (rc.right - rc.left) as u32 == monitor.region.width
                    && (rc.bottom - rc.top) as u32 == monitor.region.height
                {
                    return Ok(o);
                }

                // Fallback: match by index
                if output_index == monitor.output_index {
                    return Ok(o);
                }
            }
            Err(_) => break,
        }
        output_index += 1;
    }

    Err(anyhow::anyhow!("Could not find DXGI output matching monitor {}", monitor.output_name.trim()))
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
