use anyhow::{Context, Result};
use log::info;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Com::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::model::{MonitorInfo, Region};

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap_or_default(); }
    let mut monitors = Vec::new();
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1().context("DXGI factory failed")? };
    let mut ai = 0u32;
    loop {
        let adapter = match unsafe { factory.EnumAdapters1(ai) } { Ok(a) => a, _ => break };
        let desc = unsafe { adapter.GetDesc1() }.unwrap_or_default();
        let aname = String::from_utf16_lossy(&desc.Description);
        let mut oi = 0u32;
        loop {
            let output = match unsafe { adapter.EnumOutputs(oi) } { Ok(o) => o, _ => break };
            let od = unsafe { output.GetDesc() }.unwrap_or_default();
            let oname = String::from_utf16_lossy(&od.DeviceName);
            let rc = od.DesktopCoordinates;
            let r = Region::new(rc.left, rc.top, (rc.right-rc.left)as u32, (rc.bottom-rc.top)as u32);
            let prim = rc.left == 0 && rc.top == 0;
            let virt = !prim && detect_virtual(&oname, &aname, &r);
            monitors.push(MonitorInfo { hmonitor:0, adapter_name:aname.clone(), output_name:oname,
                description:format!("{} on {}", oname.trim(), aname), region:r, is_primary:prim,
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

fn detect_virtual(name: &str, adapter: &str, region: &Region) -> bool {
    let nl = name.to_lowercase(); let al = adapter.to_lowercase();
    for kw in &["virtual","dummy","parsec","teamviewer","anydesk","remote","spacedesk","duet","airplay","miracast","idd","usb display","displaylink"] {
        if nl.contains(kw) || al.contains(kw) { return true; }
    }
    [(1024,768),(1280,720),(1366,768),(1920,1080),(2560,1440),(3840,2160)].contains(&(region.width,region.height))
}

fn enumerate_gdi() -> Result<Vec<MonitorInfo>> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();
    unsafe {
        extern "system" fn ep(hmonitor: HMONITOR, _hdc: HDC, _rect: *mut RECT, lparam: isize) -> BOOL {
            let ms = unsafe { &mut *(lparam as *mut Vec<MonitorInfo>) };
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(hmonitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
                let rc = info.monitorInfo.rcMonitor;
                ms.push(MonitorInfo {
                    hmonitor: hmonitor.0 as u64,
                    adapter_name: String::new(),
                    output_name: String::from_utf16_lossy(&info.szDevice),
                    description: "GDI Monitor".into(),
                    region: Region::new(rc.left, rc.top, (rc.right-rc.left)as u32, (rc.bottom-rc.top)as u32),
                    is_primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
                    is_virtual_suspect: false, output_index:0, adapter_index:0,
                });
            }
            BOOL(1)
        }
        let cb: MONITORENUMPROC = Some(ep);
        EnumDisplayMonitors(None, None, cb, &mut monitors as *mut _ as isize).ok().context("EnumDisplayMonitors failed")?;
    }
    Ok(monitors)
}

pub fn find_monitor(hmonitor: u64) -> Result<Option<MonitorInfo>> {
    Ok(enumerate_monitors()?.into_iter().find(|m| m.hmonitor == hmonitor))
}
