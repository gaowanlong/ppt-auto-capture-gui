use anyhow::{Context, Result};
use windows::Win32::Foundation::{BOOL, CloseHandle, RECT, HRESULT};
use windows::Win32::Graphics::Gdi::{
    HMONITOR, HDC, EnumDisplayMonitors, GetMonitorInfoW, MONITORINFOEXW,
    MONITORINFO, MONITORINFOF_PRIMARY, MONITORENUMPROC,
};
use windows::Win32::Graphics::Dxgi::{IDXGIFactory1, IDXGIAdapter, IDXGIOutput, CreateDXGIFactory1};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;
use crate::model::{MonitorInfo, Region};

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).unwrap_or_default(); }
    let mut monitors = Vec::new();
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1().context("CreateDXGIFactory1")? };
    let mut ai = 0u32;
    loop {
        let adapter = match unsafe { factory.EnumAdapters1(ai) } { Ok(a) => a, Err(_) => break };
        let desc = unsafe { adapter.GetDesc1() }.unwrap_or_default();
        let aname = String::from_utf16_lossy(&desc.Description);
        let mut oi = 0u32;
        loop {
            let output = match unsafe { adapter.EnumOutputs(oi) } { Ok(o) => o, Err(_) => break };
            let od = unsafe { output.GetDesc() }.unwrap_or_default();
            let oname = String::from_utf16_lossy(&od.DeviceName);
            let rc = od.DesktopCoordinates;
            let r = Region::new(rc.left, rc.top, (rc.right-rc.left)as u32, (rc.bottom-rc.top)as u32);
            let prim = rc.left == 0 && rc.top == 0;
            let virt = !prim && (oname.to_lowercase().contains("virtual") || aname.to_lowercase().contains("virtual"));
            monitors.push(MonitorInfo { hmonitor:0, adapter_name:aname.clone(), output_name:oname,
                description:format!("{}x{}", r.width, r.height), region:r, is_primary:prim,
                is_virtual_suspect:virt, output_index:oi, adapter_index:ai });
            oi += 1;
        }
        ai += 1;
    }
    let gdi = enumerate_gdi()?;
    for m in &mut monitors {
        if let Some(g) = gdi.iter().find(|g| g.region.x==m.region.x && g.region.y==m.region.y) {
            m.hmonitor = g.hmonitor;
        }
    }
    if monitors.is_empty() { return Ok(gdi); }
    Ok(monitors)
}

fn enumerate_gdi() -> Result<Vec<MonitorInfo>> {
    let mut ms: Vec<MonitorInfo> = Vec::new();
    unsafe {
        extern "system" fn ep(hmon: HMONITOR, _hdc: HDC, _rect: *mut RECT, lp: isize) -> BOOL {
            let ms = &mut *(lp as *mut Vec<MonitorInfo>);
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
                let rc = info.monitorInfo.rcMonitor;
                ms.push(MonitorInfo {
                    hmonitor: hmon.0 as u64, adapter_name: String::new(),
                    output_name: String::from_utf16_lossy(&info.szDevice), description: "GDI".into(),
                    region: Region::new(rc.left, rc.top, (rc.right-rc.left)as u32, (rc.bottom-rc.top)as u32),
                    is_primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
                    is_virtual_suspect: false, output_index:0, adapter_index:0,
                });
            }
            BOOL(1)
        }
        EnumDisplayMonitors(None, None, Some(ep), &mut ms as *mut _ as isize).ok().context("EnumDisplayMonitors")?;
    }
    Ok(ms)
}

pub fn find_monitor(hmonitor: u64) -> Result<Option<MonitorInfo>> {
    Ok(enumerate_monitors()?.into_iter().find(|m| m.hmonitor == hmonitor))
}
