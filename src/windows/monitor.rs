use anyhow::{Context, Result};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{HMONITOR, HDC, EnumDisplayMonitors,
    GetMonitorInfoW, MONITORINFOEXW, MONITORINFO, MONITORENUMPROC};
use windows::Win32::Graphics::Dxgi::{IDXGIFactory1, IDXGIAdapter, IDXGIOutput, CreateDXGIFactory1};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use crate::model::{MonitorInfo, Region};

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let mut ms = Vec::new();
    if let Ok(factory) = unsafe { CreateDXGIFactory1::<IDXGIFactory1>() } {
        let mut ai = 0u32;
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(ai) } {
            if let Ok(desc) = unsafe { adapter.GetDesc1() } {
                let aname = String::from_utf16_lossy(&desc.Description);
                let mut oi = 0u32;
                while let Ok(output) = unsafe { adapter.EnumOutputs(oi) } {
                    if let Ok(od) = unsafe { output.GetDesc() } {
                        let rc = od.DesktopCoordinates;
                        let oname = String::from_utf16_lossy(&od.DeviceName);
                        ms.push(MonitorInfo{
                            hmonitor:0, adapter_name:aname.clone(), output_name:oname,
                            description:format!("{}x{}",rc.right-rc.left,rc.bottom-rc.top),
                            region:Region::new(rc.left,rc.top,(rc.right-rc.left)as u32,(rc.bottom-rc.top)as u32),
                            is_primary:rc.left==0&&rc.top==0, is_virtual_suspect:false,
                            output_index:oi, adapter_index:ai,
                        });
                    }
                    oi += 1;
                }
            }
            ai += 1;
        }
    }
    // GDI enum for HMONITOR — use transmute to avoid BOOL type issues
    let gdi = gdi_enum()?;
    for m in &mut ms {
        if let Some(g) = gdi.iter().find(|g| g.region.x==m.region.x && g.region.y==m.region.y) {
            m.hmonitor = g.hmonitor;
        }
    }
    if ms.is_empty() { Ok(gdi) } else { Ok(ms) }
}

fn gdi_enum() -> Result<Vec<MonitorInfo>> {
    let mut ms = Vec::new();
    unsafe {
        // Use i32 return via transmute to avoid needing Foundation::BOOL in callbacks
        unsafe extern "system" fn ep(hmon: HMONITOR, _hdc: HDC, _r: *const std::ffi::c_void, lp: isize) -> i32 {
            let ms = &mut *(lp as *mut Vec<MonitorInfo>);
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(hmon, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
                let rc = info.monitorInfo.rcMonitor;
                ms.push(MonitorInfo{
                    hmonitor: hmon.0 as u64, adapter_name:String::new(),
                    output_name: String::from_utf16_lossy(&info.szDevice), description:"GDI".into(),
                    region:Region::new(rc.left,rc.top,(rc.right-rc.left)as u32,(rc.bottom-rc.top)as u32),
                    is_primary:(info.monitorInfo.dwFlags & 1)!=0, is_virtual_suspect:false,
                    output_index:0, adapter_index:0,
                });
            }
            1i32
        }
        let cb: MONITORENUMPROC = std::mem::transmute(ep as extern "system" fn(_,_,_,_) -> i32);
        EnumDisplayMonitors(None, None, cb, &mut ms as *mut _ as isize).ok()
            .context("EnumDisplayMonitors")?;
    }
    Ok(ms)
}

pub fn find_monitor(hmonitor: u64) -> Result<Option<MonitorInfo>> {
    Ok(enumerate_monitors()?.into_iter().find(|m| m.hmonitor == hmonitor))
}
