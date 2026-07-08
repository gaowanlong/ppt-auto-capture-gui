//! Enumerate monitors/adapters using DXGI and GDI.
//! Detects physical displays, HDMI dummy plugs, and virtual displays.

use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::System::Com::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::model::{MonitorInfo, Region};

/// Enumerate all connected monitors using DXGI.
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    // Initialize COM
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .unwrap_or_default();
    }

    let mut monitors = Vec::new();

    // Create DXGI factory
    let factory: IDXGIFactory1 = unsafe {
        CreateDXGIFactory1().context("Failed to create DXGI factory")?
    };

    // Enumerate adapters
    let mut adapter_index: u32 = 0;
    loop {
        let adapter_result = unsafe { factory.EnumAdapters1(adapter_index) };
        let adapter = match adapter_result {
            Ok(a) => a,
            Err(_) => break,
        };

        let adapter_desc = unsafe { adapter.GetDesc1() }.unwrap_or_default();
        let adapter_name = String::from_utf16_lossy(&adapter_desc.Description);

        // Enumerate outputs (monitors) on this adapter
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

            // Determine if primary
            let is_primary = desktop_coords.left == 0 && desktop_coords.top == 0;

            // Detect if likely virtual display:
            // - Non-primary
            // - Resolution is something common for dummy plugs (1024x768, 1920x1080, 3840x2160)
            // - Name doesn't contain typical physical monitor brand names
            let is_virtual_suspect = detect_virtual_suspect(&output_name, &adapter_name, &region, is_primary);

            let mon_info = MonitorInfo {
                hmonitor: 0, // Will be filled from GDI enumeration
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

    // Also enumerate via GDI to get HMONITOR handles
    let gdi_monitors = enumerate_monitors_gdi()?;
    for mon in &mut monitors {
        if let Some(gdi_mon) = gdi_monitors.iter().find(|gm| {
            gm.region.x == mon.region.x && gm.region.y == mon.region.y
        }) {
            mon.hmonitor = gdi_mon.hmonitor;
        }
    }

    // If DXGI found nothing, use GDI results directly
    if monitors.is_empty() {
        monitors = gdi_monitors;
    }

    Ok(monitors)
}

/// Detect if a display is likely a virtual or dummy display.
fn detect_virtual_suspect(output_name: &str, adapter_name: &str, region: &Region, is_primary: bool) -> bool {
    if is_primary {
        return false;
    }

    // Virtual displays often show up without a physical EDID string
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

    // Common dummy HDMI plug resolutions
    let dummy_resolutions = [
        (1024, 768),
        (1280, 720),
        (1366, 768),
        (1920, 1080),
        (2560, 1440),
        (3840, 2160),
        (1600, 900),
        (1440, 900),
    ];

    let (w, h) = (region.width, region.height);
    if dummy_resolutions.contains(&(w, h)) {
        return true;
    }

    false
}

/// Enumerate monitors via GDI to get HMONITOR handles.
fn enumerate_monitors_gdi() -> Result<Vec<MonitorInfo>> {
    let mut monitors = Vec::new();

    unsafe {
        let enum_proc: MONITORENUMPROC = Some(monitor_enum_proc);
        let ctx = &mut monitors as *mut Vec<MonitorInfo> as isize;
        EnumDisplayMonitors(None, None, enum_proc, ctx)
            .ok()
            .context("EnumDisplayMonitors failed")?;
    }

    Ok(monitors)
}

extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    hdc: HDC,
    _rect: *mut RECT,
    lparam: isize,
) -> i32 {
    unsafe {
        let monitors = &mut *(lparam as *mut Vec<MonitorInfo>);

        let mut monitor_info = MONITORINFOEXW::default();
        monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmonitor, &mut monitor_info as *mut MONITORINFOEXW as *mut MONITORINFO).is_ok() {
            let rc = monitor_info.monitorInfo.rcMonitor;
            let dev_name = String::from_utf16_lossy(&monitor_info.szDevice);
            let is_primary = (monitor_info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;

            let region = Region::new(
                rc.left, rc.top,
                (rc.right - rc.left) as u32,
                (rc.bottom - rc.top) as u32,
            );

            let mon_info = MonitorInfo {
                hmonitor: hmonitor.0 as u64,
                adapter_name: String::new(),
                output_name: dev_name,
                description: format!("GDI Monitor {} ({})", dev_name.trim(),
                    if is_primary { "Primary" } else { "Secondary" }),
                region,
                is_primary,
                is_virtual_suspect: false, // Will be refined when merged with DXGI results
                output_index: 0,
                adapter_index: 0,
            };

            monitors.push(mon_info);
        }
        1 // Continue enumeration
    }
}

/// Find a specific monitor by HMONITOR.
pub fn find_monitor(hmonitor: u64) -> Result<Option<MonitorInfo>> {
    let monitors = enumerate_monitors()?;
    Ok(monitors.into_iter().find(|m| m.hmonitor == hmonitor))
}

/// Find a monitor by index in the list.
pub fn get_monitor_by_index(index: usize) -> Result<Option<MonitorInfo>> {
    let monitors = enumerate_monitors()?;
    Ok(monitors.into_iter().nth(index))
}
