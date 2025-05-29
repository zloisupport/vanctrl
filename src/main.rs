#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use ini::Ini;
use std::path::Path;
use std::process::{Command,Stdio};
use std::{env, io};
slint::include_modules!();

fn main() {
    const REGISTER_PATH: &str = "HKLM:/Software/Microsoft/Windows/CurrentVersion/Run";

    let app = AppWindow::new().unwrap();

    let weak = app.as_weak();
    let mut tray = String::new();

    let mut conf = Ini::load_from_file("conf.ini").unwrap_or_else(|_| Ini::new());

    if let Some(section) = &conf.section(Some("Vanguard")) {
        if let Some(state_str) = section.get("state") {
            println!("state");
            let app = weak.upgrade().unwrap();
            let active = state_str == "true";
            app.set_active(active);
        }
        tray = load_tray_path(section);
    }

    app.on_add_to_autostart(|| {});

    app.on_change_active(move || {
        // let app=  weak.upgrade().unwrap();
        // let active_status = app.get_active();
    });

    app.on_save_config(move || {
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
    });
    fn load_tray_path(section: &ini::Properties) -> String {
        match section.get("vg_tray") {
            Some(vg_tray) if !vg_tray.trim().is_empty() => vg_tray.to_string(),
            _ => {
                let mut tray: String = read_tray_path(&REGISTER_PATH).unwrap_or_default();
                if tray.trim().is_empty() {
                    tray = load_vg_tray_path();
                }
                tray
            }
        }
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

    fn read_tray_path(register_path: &str) -> io::Result<String> {
        let powershell_cmd = format!(
            "(Get-ItemProperty -Path \"{}\").'Riot Vanguard'",
            register_path
        );
        println!("{}", powershell_cmd);
        let output = Command::new("powershell")
            .args(["-Command", &powershell_cmd])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string()
                .replace("\"", ""))
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn run_activate(tray: &str) {
        run_command_cli("sc config vgc start= demand & sc config vgk start= system");

        run_command_cli("net start vgc");

        run_command(&tray);

        let command = format!(
            "powershell Set-ItemProperty -Path {} -Name 'Riot Vanguard' -Value '{}'",
            REGISTER_PATH, &tray
        );
        run_command_cli(&command);
    }

    fn run_command_cli(program: &str) {
        let command = program;
        let mut process: Command = Command::new("cmd");
        process.args(["/c", &command])
        .stdout(Stdio::null()) // скрыть стандартный вывод
        .stderr(Stdio::null()) // скрыть стандартный вывод ошибок
        .stdin(Stdio::null()); // опционально: скрыть ввод


        let child = process.spawn();
        let child = match child {
            Ok(child) => child,
            Err(err) => {
                println!("{}", err);
                return;
            }
        };
    }

    fn run_command(program: &str) {
        let mut proccess: Command = Command::new(program);

        proccess.args(["-WindowStyle Hidden"]);

        let child = proccess.spawn();

        if child.is_ok() {
            
        } else {
            
        }
    }

    fn run_deactivate() {
        run_command_cli(
            "sc config vgc start= disabled & sc config vgk start= disabled & net stop vgc & net stop vgk",
        );
        let command = format!(
            "powershell Remove-ItemProperty -Path {} -Name 'Riot Vanguard'",
            REGISTER_PATH
        );
        run_command_cli(&command);
        run_command_cli("taskkill /IM vgtray.exe")
    }

    app.run().unwrap();
}
