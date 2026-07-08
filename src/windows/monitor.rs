use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Com::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::model::{MonitorInfo, Region};

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap_or_default();
    }

    let mut monitors = Vec::new();

    let factory: IDXGIFactory1 = unsafe {
        CreateDXGIFactory1().context("Failed to create DXGI factory")?
    };

    let mut adapter_index: u32 = 0;
    loop {
        let adapter_result = unsafe { factory.EnumAdapters1(adapter_index) };
        let adapter = match adapter_result {
            Ok(a) => a,
            Err(_) => break,
        };

        let adapter_desc = unsafe { adapter.GetDesc1() }.unwrap_or_default();
        let adapter_name = String::from_utf16_lossy(&adapter_desc.Description);

        let mut output_index: u32 = 0;
        loop {
            let output_result = unsafe { adapter.EnumOutputs(output_index) };
            let output = match output_result {
                Ok(o) => o,
                Err(_) => break,
            };

            let output_desc = unsafe { output.GetDesc() }.unwrap_or_default();
            let output_name = String::from_utf16_lossy(&output_desc.DeviceName);
            let desktop_coords = output_desc.DesktopCoordinates;

            let region = Region::new(
                desktop_coords.left,
                desktop_coords.top,
                (desktop_coords.right - desktop_coords.left) as u32,
                (desktop_coords.bottom - desktop_coords.top) as u32,
            );

            let is_primary = desktop_coords.left == 0 && desktop_coords.top == 0;
            let is_virtual_suspect = detect_virtual_suspect(&output_name, &adapter_name, &region, is_primary);

            let mon_info = MonitorInfo {
                hmonitor: 0,
                adapter_name: adapter_name.clone(),
                output_name: output_name.clone(),
                description: format!("{} on {}", output_name.trim(), adapter_name),
                region,
                is_primary,
                is_virtual_suspect,
                output_index,
                adapter_index,
            };

            info!("Found monitor: {} ({}x{}) virtual={}",
                output_name.trim(), region.width, region.height, is_virtual_suspect);

            monitors.push(mon_info);
            output_index += 1;
        }
        adapter_index += 1;
    }

    // GDI fallback to get HMONITOR handles using raw Win32 API
    let gdi_monitors = enumerate_monitors_gdi()?;
    for mon in &mut monitors {
        if let Some(gdi_mon) = gdi_monitors.iter().find(|gm| {
            gm.region.x == mon.region.x && gm.region.y == mon.region.y
        }) {
            mon.hmonitor = gdi_mon.hmonitor;
        }
    }

    if monitors.is_empty() {
        monitors = gdi_monitors;
    }

    Ok(monitors)
}

fn detect_virtual_suspect(output_name: &str, adapter_name: &str, region: &Region, is_primary: bool) -> bool {
    if is_primary {
        return false;
    }

    let name_lower = output_name.to_lowercase();
    let adapter_lower = adapter_name.to_lowercase();

    let virtual_keywords = [
        "virtual", "dummy", "vga", "parsec", "teamviewer", "anydesk",
        "remote", "spacedesk", "duet", "airplay", "miracast", "idd",
        "usb display", "displaylink",
    ];

    for kw in &virtual_keywords {
        if name_lower.contains(kw) || adapter_lower.contains(kw) {
            return true;
        }
    }

    let dummy_resolutions = [
        (1024, 768), (1280, 720), (1366, 768), (1920, 1080),
        (2560, 1440), (3840, 2160), (1600, 900), (1440, 900),
    ];

    let (w, h) = (region.width, region.height);
    if dummy_resolutions.contains(&(w, h)) {
        return true;
    }

    false
}

fn enumerate_monitors_gdi() -> Result<Vec<MonitorInfo>> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();

    unsafe {
        // Use raw function pointers for the enum callback
        extern "system" fn enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            lparam: isize,
        ) -> BOOL {
            let monitors = unsafe { &mut *(lparam as *mut Vec<MonitorInfo>) };

            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

            let ok = GetMonitorInfoW(
                hmonitor,
                &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
            );

            if ok.is_ok() {
                let rc = info.monitorInfo.rcMonitor;
                let dev_name = String::from_utf16_lossy(&info.szDevice);
                let is_primary = (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;

                let region = Region::new(
                    rc.left, rc.top,
                    (rc.right - rc.left) as u32,
                    (rc.bottom - rc.top) as u32,
                );

                monitors.push(MonitorInfo {
                    hmonitor: hmonitor.0 as u64,
                    adapter_name: String::new(),
                    output_name: dev_name,
                    description: format!("GDI Monitor"),
                    region,
                    is_primary,
                    is_virtual_suspect: false,
                    output_index: 0,
                    adapter_index: 0,
                });
            }

            TRUE
        }

        let cb: MONITORENUMPROC = Some(enum_proc);
        let ctx = &mut monitors as *mut Vec<MonitorInfo> as isize;
        EnumDisplayMonitors(None, None, cb, ctx)
            .ok()
            .context("EnumDisplayMonitors failed")?;
    }

    Ok(monitors)
}

pub fn find_monitor(hmonitor: u64) -> Result<Option<MonitorInfo>> {
    let monitors = enumerate_monitors()?;
    Ok(monitors.into_iter().find(|m| m.hmonitor == hmonitor))
}

pub fn get_monitor_by_index(index: usize) -> Result<Option<MonitorInfo>> {
    let monitors = enumerate_monitors()?;
    Ok(monitors.into_iter().nth(index))
}
