//! Query desktop availability synchronously: no detached polling threads or growing queues.
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, GetThreadDesktop, GetUserObjectInformationW, OpenInputDesktop,
    DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, HDESK, UOI_NAME,
};
use windows::Win32::System::Threading::GetCurrentThreadId;

#[link(name = "wtsapi32")]
extern "system" {
    fn WTSQuerySessionInformationW(
        server: isize,
        session: u32,
        class: u32,
        buffer: *mut *mut std::ffi::c_void,
        bytes: *mut u32,
    ) -> i32;
    fn WTSFreeMemory(memory: *mut std::ffi::c_void);
}

fn desktop_name(desktop: HDESK) -> Option<Vec<u16>> {
    let mut name = [0u16; 256];
    unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            std::mem::size_of_val(&name) as u32,
            None,
        )
        .ok()?;
    }
    Some(name[..name.iter().position(|&c| c == 0)?].to_vec())
}

pub fn input_desktop_available() -> bool {
    unsafe {
        // OpenInputDesktop alone also succeeds for disconnected RDP sessions.
        let mut buffer = std::ptr::null_mut();
        let mut bytes = 0;
        if WTSQuerySessionInformationW(0, u32::MAX, 8, &mut buffer, &mut bytes) == 0 {
            return false;
        }
        let active = !buffer.is_null() && bytes >= 4 && *(buffer as *const i32) == 0;
        WTSFreeMemory(buffer);
        if !active {
            return false;
        }
        let Ok(input) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
        else {
            return false;
        };
        let input_name = desktop_name(input);
        let own_name = GetThreadDesktop(GetCurrentThreadId())
            .ok()
            .and_then(desktop_name);
        let _ = CloseDesktop(input);
        input_name.is_some() && input_name == own_name
    }
}

pub fn log_capture_resources() {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS};
    unsafe {
        let process = GetCurrentProcess();
        let mut memory = PROCESS_MEMORY_COUNTERS::default();
        let size = std::mem::size_of_val(&memory) as u32;
        memory.cb = size;
        if GetProcessMemoryInfo(process, &mut memory, size).is_ok() {
            log::info!(
                "Capture health: GDI objects={}, working_set_mib={}, peak_working_set_mib={}",
                GetGuiResources(process, GR_GDIOBJECTS),
                memory.WorkingSetSize / 1024 / 1024,
                memory.PeakWorkingSetSize / 1024 / 1024
            );
        }
    }
}
