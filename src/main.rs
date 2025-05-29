#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use ini::Ini;
use slint::SharedString;
use std::ffi::CString;
use std::path::Path;
use std::ptr;
use std::{env, io};
use winapi::um::handleapi::CloseHandle;
use winapi::um::shellapi::ShellExecuteW;
use winapi::um::tlhelp32::{
    CreateToolhelp32Snapshot, PROCESSENTRY32, Process32First, Process32Next, TH32CS_SNAPPROCESS,
};
use winapi::um::winuser::SW_HIDE;
use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_WRITE};
slint::include_modules!();

fn main() {
    let app = AppWindow::new().unwrap();

    let weak = app.as_weak();
    let mut tray = String::new();

    let mut conf = Ini::load_from_file("conf.ini").unwrap_or_else(|_| Ini::new());

    if let Some(section) = &conf.section(Some("Vanguard")) {
        tray = load_tray_path(section);
    }

    let process_activity_status = checking_process();

    let app = weak.upgrade().unwrap();

    if process_activity_status {
        app.set_active(true);
    } else {
        app.set_active(false);
    }

    let autostart_status = check_to_autostart();
    if autostart_status {
        app.set_autostart(SharedString::from("Remove from autostart"));
    } else {
        app.set_autostart(SharedString::from("Add to autostart"));
    }
    app.on_bug_report(move || {
        launch_exe("start https://github.com/zloisupport/vanctrl/issues");
    });

    app.on_add_to_autostart({
        let weak = weak.clone();
        move || {
            let app = weak.upgrade().unwrap();
            let autostart_status = check_to_autostart();
            if !autostart_status {
                if let Err(e) = add_to_autostart() {
                    println!("Failed to add to autostart: {}", e);
                } else {
                    app.set_autostart(SharedString::from("Remove from autostart"));
                }
            } else {
                if let Err(e) = remove_to_autostart() {
                    println!("Failed to add to autostart: {}", e);
                } else {
                    app.set_autostart(SharedString::from("Add to autostart"));
                }
            }
        }
    });

    app.on_save_config({
        let weak = weak.clone();
        move || {
            let app = weak.upgrade().unwrap();
            let active_status = app.get_active();

            if active_status {
                run_activate(&tray);
            } else {
                run_deactivate();
            }

            conf.with_section(Some("Vanguard"))
                .set("vgc_tray", &tray)
                .set("state", active_status.to_string());
            conf.write_to_file("conf.ini").unwrap();
        }
    });

    fn load_tray_path(section: &ini::Properties) -> String {
        match section.get("vgc_tray") {
            Some(vg_tray) if !vg_tray.trim().is_empty() => vg_tray.to_string(),
            _ => match get_tray_path() {
                Ok(p) => p,
                _ => load_vg_tray_path(),
            },
        }
    }

    fn check_to_autostart() -> bool {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        match hklm.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run") {
            Ok(subkey) => subkey.get_raw_value("VG Out").is_ok(),
            Err(_) => false,
        }
    }

    fn add_to_autostart() -> Result<(), io::Error> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        match env::current_exe() {
            Ok(exe_path) => {
                let curver_run = hklm.open_subkey_with_flags(
                    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
                    KEY_WRITE,
                )?;
                curver_run.set_value("VG Out", &exe_path.as_path().as_os_str())?;
            }
            Err(e) => println!("Failed to get current executable path: {}", e),
        }

        Ok(())
    }

    fn remove_to_autostart() -> Result<(), io::Error> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        let curver_run = hklm.open_subkey_with_flags(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_WRITE,
        )?;
        curver_run.delete_value("VG Out")?;

        Ok(())
    }

    fn load_vg_tray_path() -> String {
        match env::var("SystemRoot") {
            Ok(sys_drive) => {
                let win_dir = &sys_drive.replace("Windows", "");
                let vanguard_path = Path::new(win_dir)
                    .join("Program Files")
                    .join("Riot Vanguard")
                    .join("vgtray.exe");
                if vanguard_path.exists() {
                    return vanguard_path
                        .to_str()
                        .unwrap_or("C:\\Program Files\\Riot Vanguard\\vgtray.exe")
                        .to_string();
                }
            }
            Err(err) => {
                println!("{}", err)
            }
        }
        String::new()
    }

    fn get_tray_path() -> Result<String, io::Error> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let curver_run = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run")?;
        let riot_vanguard: String = curver_run.get_value("Riot Vanguard")?;
        Ok(riot_vanguard)
    }

    fn set_tray_path(path_to_tray: &str) -> Result<(), io::Error> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let curver_run = hklm.open_subkey_with_flags(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_WRITE,
        )?;
        curver_run.set_value("Riot Vanguard", &path_to_tray)?;
        Ok(())
    }

    fn delete_tray_path() -> Result<(), io::Error> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let curver_run = hklm.open_subkey_with_flags(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_WRITE,
        )?;
        curver_run.delete_value("Riot Vanguard")?;
        Ok(())
    }

    fn get_process_id_by_name(process_name: &str) -> Option<u32> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == winapi::um::handleapi::INVALID_HANDLE_VALUE {
                return None;
            }

            let mut entry: PROCESSENTRY32 = std::mem::zeroed();

            entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

            if Process32First(snapshot, &mut entry) == 0 {
                CloseHandle(snapshot);
                return None;
            }

            let target_name = CString::new(process_name).unwrap();
            loop {
                let exe_name = std::ffi::CStr::from_ptr(entry.szExeFile.as_ptr());
                if exe_name == target_name.as_c_str() {
                    CloseHandle(snapshot);
                    return Some(entry.th32ProcessID);
                }

                if Process32Next(snapshot, &mut entry) == 0 {
                    break;
                }
            }

            CloseHandle(snapshot);
            None
        }
    }

    fn checking_process() -> bool {
        let process_name: Vec<&str> = vec!["vgtray.exe", "vgc.exe"];
        for name in &process_name {
            match get_process_id_by_name(&name.to_ascii_lowercase()) {
                Some(_process_id) => {
                    return true;
                }
                None => println!("Process not found {}", name.to_string()),
            }
        }

        false
    }

    fn run_activate(tray: &str) {
        launch_exe("sc config vgc start= demand & sc config vgk start= system");
        launch_exe("net start vgc");
        launch_exe(&tray);
        if let Err(e) = set_tray_path(&tray) {
            println!("Failed to set tray path: {}", e);
        }
    }

    fn launch_exe(cmd: &str) {
        let operation = to_wide_ptr("runas");
        let file = to_wide_ptr("cmd.exe");
        let command = format!("/c \"{}\"", &cmd);
        let params = to_wide_ptr(&command);
        let dir = ptr::null();

        unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                params.as_ptr(),
                dir,
                SW_HIDE,
            );
        }
    }
    fn to_wide_ptr(s: &str) -> Vec<u16> {
        use std::os::windows::prelude::*;
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(Some(0))
            .collect()
    }

    fn run_deactivate() {
        launch_exe(
            "sc config vgc start= disabled & sc config vgk start= disabled & net stop vgc & net stop vgk",
        );

        launch_exe("taskkill /F /Im vgtray.exe");
        if let Err(e) = delete_tray_path() {
            println!("Failed to delete tray path: {}", e);
        }
    }

    app.run().unwrap();
}
