use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::System::Com::*;
use windows::core::IUnknown;
use crate::model::{Frame, MonitorInfo};

pub struct DxgiCapturer {
    device: Option<ID3D11Device>,
    context: Option<ID3D11DeviceContext>,
    duplication: Option<IDXGIOutputDuplication>,
    monitor_info: Option<MonitorInfo>,
    frame_index: u64,
}

impl DxgiCapturer {
    pub fn new() -> Self { Self { device: None, context: None, duplication: None, monitor_info: None, frame_index: 0 } }

    pub fn initialize(&mut self, monitor: &MonitorInfo) -> Result<()> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap_or_default(); }
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature: D3D_FEATURE_LEVEL = D3D_FEATURE_LEVEL::default();
        unsafe {
            D3D11CreateDevice(
                None, D3D_DRIVER_TYPE_HARDWARE, None, D3D11_CREATE_DEVICE_FLAG(0),
                std::ptr::null(), 0, D3D11_SDK_VERSION,
                &mut device as *mut Option<ID3D11Device>,
                &mut feature as *mut D3D_FEATURE_LEVEL,
                &mut context as *mut Option<ID3D11DeviceContext>,
            )
        }.ok().context("D3D11CreateDevice")?;
        let device = device.context("device")?;
        let context = context.context("context")?;
        let dxgi_device: IDXGIDevice = unsafe { device.cast() }.context("cast DXGIDevice")?;
        let adapter = unsafe { dxgi_device.GetAdapter() }.context("GetAdapter")?;
        let output = find_output(&adapter, monitor)?;
        let output1: IDXGIOutput1 = unsafe { output.cast() }.context("cast Output1")?;
        let unknown: IUnknown = unsafe { device.cast() }.context("cast IUnknown")?;
        let duplication = unsafe { output1.DuplicateOutput(&unknown) }.context("DuplicateOutput")?;
        self.device = Some(device);
        self.context = Some(context);
        self.duplication = Some(duplication);
        self.monitor_info = Some(monitor.clone());
        self.frame_index = 0;
        info!("DXGI capturer initialized");
        Ok(())
    }

    pub fn capture_frame(&mut self, timeout_ms: u32) -> Result<Option<Frame>> {
        let duplication = self.duplication.as_ref().context("not init")?;
        let mut fi = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        match unsafe { duplication.AcquireNextFrame(timeout_ms, &mut fi, &mut resource) } {
            Ok(()) => {}
            Err(e) => {
                if e.code() == DXGI_ERROR_WAIT_TIMEOUT.0 { return Ok(None); }
                if e.code() == DXGI_ERROR_ACCESS_LOST.0 { return Err(anyhow::anyhow!("DXGI access lost")); }
                return Err(anyhow::anyhow!("AcquireNextFrame: {:?}", e));
            }
        }
        let resource = resource.context("no resource")?;
        // Get surface from the shared resource for CPU readback
        let surface: IDXGISurface1 = unsafe { resource.cast() }.context("cast surface")?;
        // Get the desc for dimensions
        let mut sdesc = DXGI_SURFACE_DESC::default();
        unsafe { surface.GetDesc(&mut sdesc) }.ok()?;
        // Map the surface for CPU read
        let mut mapped = DXGI_MAPPED_RECT::default();
        unsafe { surface.Map(&mut mapped, 1) }.ok().context("Map")?;
        let w = sdesc.Width;
        let h = sdesc.Height;
        let pitch = mapped.Pitch;
        let data_size = (pitch * h) as usize;
        let src = unsafe { std::slice::from_raw_parts(mapped.pBits, data_size) };
        let data = src.to_vec();
        unsafe { surface.Unmap().ok(); }
        if unsafe { duplication.ReleaseFrame().is_err() } { warn!("ReleaseFrame"); }
        self.frame_index += 1;
        Ok(Some(Frame::new(data, w, h, pitch, self.frame_index, now_ms())))
    }

    pub fn release(&mut self) { self.duplication = None; self.context = None; self.device = None; }
    pub fn is_initialized(&self) -> bool { self.duplication.is_some() }
}

fn find_output(adapter: &IDXGIAdapter, monitor: &MonitorInfo) -> Result<IDXGIOutput> {
    let mut i = 0u32;
    loop {
        let output = unsafe { adapter.EnumOutputs(i) };
        match output {
            Ok(o) => {
                let d = unsafe { o.GetDesc() }.unwrap_or_default();
                let rc = d.DesktopCoordinates;
                if rc.left==monitor.region.x&&rc.top==monitor.region.y
                    && (rc.right-rc.left)as u32==monitor.region.width
                    && (rc.bottom-rc.top)as u32==monitor.region.height { return Ok(o); }
                if i==monitor.output_index { return Ok(o); }
            }
            Err(_) => break,
        }
        i += 1;
    }
    Err(anyhow::anyhow!("no output found"))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
