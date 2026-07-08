use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, D3D11CreateDevice, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIAdapter, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, IDXGISurface1, DXGI_OUTDUPL_FRAME_INFO, DXGI_SURFACE_DESC,
    DXGI_MAPPED_RECT,
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_ERROR_ACCESS_LOST,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
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
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok(); }
        let mut dev: Option<ID3D11Device> = None;
        let mut ctx: Option<ID3D11DeviceContext> = None;
        unsafe {
            D3D11CreateDevice(
                None, D3D_DRIVER_TYPE_HARDWARE, None, 0u32,
                std::ptr::null(), 0u32, D3D11_SDK_VERSION,
                &mut dev as *mut Option<ID3D11Device>,
                &mut ctx as *mut Option<ID3D11DeviceContext>,
            )
        }.ok().context("D3D11CreateDevice")?;
        let device = dev.context("device")?;
        let context = ctx.context("context")?;

        // COM casts via transmute_copy (both implement IUnknown with same vtable layout)
        let dxgi_device: IDXGIDevice = unsafe { std::mem::transmute_copy(&device) };
        let adapter = unsafe { dxgi_device.GetAdapter() }.context("GetAdapter")?;
        let output = find_output(&adapter, monitor)?;

        // Cast output->output1, then device->IUnknown for DuplicateOutput
        let output1: IDXGIOutput1 = unsafe { std::mem::transmute_copy(&output) };
        let unknown: IUnknown = unsafe { std::mem::transmute_copy(&device) };

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
        let dup = self.duplication.as_ref().context("not init")?;
        let mut fi = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        match unsafe { dup.AcquireNextFrame(timeout_ms, &mut fi, &mut resource) } {
            Ok(()) => {}
            Err(e) => {
                if e.0 == DXGI_ERROR_WAIT_TIMEOUT { return Ok(None); }
                if e.0 == DXGI_ERROR_ACCESS_LOST { return Err(anyhow::anyhow!("DXGI access lost")); }
                return Err(anyhow::anyhow!("AcquireNextFrame: {:?}", e));
            }
        }
        let resource = resource.context("no resource")?;
        // Cast resource to surface via raw transmute (same COM base)
        let surface: IDXGISurface1 = unsafe { std::mem::transmute_copy(&resource) };
        let mut sd = DXGI_SURFACE_DESC::default();
        unsafe { surface.GetDesc(&mut sd).ok(); }
        // Map for CPU readback
        let mut mapped = DXGI_MAPPED_RECT::default();
        unsafe { surface.Map(&mut mapped, 1).ok().context("Map")?; }
        let w = sd.Width; let h = sd.Height;
        let data = unsafe {
            std::slice::from_raw_parts(mapped.pBits, (mapped.Pitch * h) as usize)
        }.to_vec();
        unsafe { surface.Unmap().ok(); }
        if unsafe { dup.ReleaseFrame().is_err() } { warn!("ReleaseFrame"); }
        self.frame_index += 1;
        Ok(Some(Frame::new(data, w, h, mapped.Pitch, self.frame_index, now_ms())))
    }

    pub fn release(&mut self) { self.duplication = None; self.context = None; self.device = None; }
    pub fn is_initialized(&self) -> bool { self.duplication.is_some() }
}

fn find_output(adapter: &IDXGIAdapter, mon: &MonitorInfo) -> Result<IDXGIOutput> {
    let mut i = 0u32;
    loop {
        if let Ok(o) = unsafe { adapter.EnumOutputs(i) } {
            if let Ok(d) = unsafe { o.GetDesc() } {
                let rc = d.DesktopCoordinates;
                if rc.left==mon.region.x&&rc.top==mon.region.y
                    && (rc.right-rc.left)as u32==mon.region.width
                    && (rc.bottom-rc.top)as u32==mon.region.height { return Ok(o); }
            }
            if i==mon.output_index { return Ok(o); }
        } else { break; }
        i += 1;
    }
    Err(anyhow::anyhow!("no DXGI output"))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
