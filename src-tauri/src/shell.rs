use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Serialize;
use std::io::{BufReader, Write};
use std::sync::Arc;
use tauri::{Emitter, State};

use crate::{AppState, TerminalOutput};

#[derive(Serialize, Clone)]
pub struct SshInfo {
    pub is_ssh: bool,
    pub target: String,      // user@host or empty
    pub remote_cwd: String,  // remote cwd or empty
    pub remote_os: String,   // e.g. "Linux 5.15.0-91-generic x86_64" or empty
    pub remote_shell: String, // e.g. "bash" or "zsh" or empty
}

#[tauri::command]
pub fn spawn_shell(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let pty_system = native_pty_system();

    let initial_rows = rows.unwrap_or(24);
    let initial_cols = cols.unwrap_or(80);

    let pair = pty_system
        .openpty(PtySize {
            rows: initial_rows,
            cols: initial_cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    #[cfg(unix)]
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    #[cfg(windows)]
    let shell = "powershell.exe".to_string();

    let mut cmd = CommandBuilder::new(&shell);
    #[cfg(unix)]
    cmd.arg("-l");
    #[cfg(windows)]
    {
        cmd.arg("-NoLogo");
        cmd.arg("-NoExit");
        cmd.arg("-Command");
        // Override prompt to emit OSC 7 with the current directory so the
        // frontend can track cwd changes in real time.
        cmd.arg(concat!(
            "function prompt {",
            " $e=[char]27; $b=[char]7;",
            " $p=$PWD.Path.Replace('\\','/');",
            " \"$e]7;file://localhost/$p$b\"",
            " + \"PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) \"",
            " }"
        ));
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("LANG", "en_US.UTF-8");
    #[cfg(unix)]
    {
        cmd.env("CLICOLOR", "1");
        cmd.env("CLICOLOR_FORCE", "1");
        cmd.env("LSCOLORS", "ExGxFxdaCxDaDahbadacec");
    }
    if let Some(home) = dirs::home_dir() {
        cmd.cwd(&home);
    }

    let mut child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    let child_pid = child.process_id();
    drop(pair.slave);

    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(
            session_id.clone(),
            crate::PtySession {
                writer,
                master: pair.master,
                child_pid,
            },
        );
    }

    // Reader thread: send raw PTY output to frontend (no filtering)
    let sid = session_id.clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let mut buf_reader = BufReader::new(reader);
        let mut buf = vec![0u8; 8192];
        loop {
            match std::io::Read::read(&mut buf_reader, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_handle.emit(
                        "terminal-output",
                        TerminalOutput {
                            session_id: sid.clone(),
                            data,
                        },
                    );
                }
                Err(_) => break,
            }
        }
    });

    let sid2 = session_id.clone();
    let state_clone = state.inner().clone();
    let app_handle2 = app.clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        let mut sessions = state_clone.sessions.lock().unwrap();
        sessions.remove(&sid2);
        let _ = app_handle2.emit("session-ended", sid2);
    });

    Ok(session_id)
}

#[tauri::command]
pub fn write_to_shell(
    session_id: String,
    data: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.get_mut(&session_id) {
        session
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| e.to_string())?;
        session.writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

#[tauri::command]
pub fn resize_pty(
    session_id: String,
    rows: u16,
    cols: u16,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.get(&session_id) {
        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

#[tauri::command]
pub fn get_shell_cwd(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&session_id).ok_or("Session not found")?;
    let pid = session.child_pid.ok_or("No PID available")?;

    let path = get_process_cwd(pid)?;
    Ok(replace_home_prefix(&path))
}

fn replace_home_prefix(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path == home_str {
            return "~".to_string();
        }
        if let Some(rest) = path.strip_prefix(home_str.as_ref()) {
            let result = format!("~{rest}");
            // Normalize to forward slashes for consistent frontend handling
            return result.replace('\\', "/");
        }
    }
    path.replace('\\', "/")
}

#[cfg(unix)]
fn get_process_cwd(pid: u32) -> Result<String, String> {
    let output = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                return Ok(path.to_string());
            }
        }
    }
    Err("Could not determine CWD".to_string())
}

#[cfg(windows)]
fn get_process_cwd(pid: u32) -> Result<String, String> {
    use sysinfo::{Pid, ProcessRefreshKind, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::new().with_cwd(UpdateKind::Always),
    );
    if let Some(proc) = sys.process(Pid::from_u32(pid)) {
        if let Some(cwd) = proc.cwd() {
            return Ok(cwd.to_string_lossy().to_string());
        }
    }
    Err("Could not determine CWD".to_string())
}

#[tauri::command]
pub fn detect_ssh(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SshInfo, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&session_id).ok_or("Session not found")?;
    let pid = session.child_pid.ok_or("No PID available")?;

    detect_ssh_for_pid(pid)
}

#[cfg(unix)]
fn detect_ssh_for_pid(pid: u32) -> Result<SshInfo, String> {
    let output = std::process::Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for child_pid_str in stdout.lines() {
        let child_pid = child_pid_str.trim();
        if child_pid.is_empty() {
            continue;
        }

        let ps_output = std::process::Command::new("ps")
            .args(["-p", child_pid, "-o", "command="])
            .output();

        if let Ok(ps_out) = ps_output {
            let cmd_line = String::from_utf8_lossy(&ps_out.stdout).trim().to_string();
            if cmd_line.starts_with("ssh ") || cmd_line.contains("/ssh ") {
                let target = parse_ssh_target(&cmd_line);
                if !target.is_empty() {
                    let remote_cwd = get_remote_cwd(&target).unwrap_or_default();
                    let remote_os = get_remote_os(&target).unwrap_or_default();
                    let remote_shell = get_remote_shell(&target).unwrap_or_default();
                    return Ok(SshInfo {
                        is_ssh: true,
                        target,
                        remote_cwd,
                        remote_os,
                        remote_shell,
                    });
                }
            }
        }
    }

    Ok(SshInfo {
        is_ssh: false,
        target: String::new(),
        remote_cwd: String::new(),
        remote_os: String::new(),
        remote_shell: String::new(),
    })
}

#[cfg(windows)]
fn detect_ssh_for_pid(pid: u32) -> Result<SshInfo, String> {
    use sysinfo::{Pid, System};
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let parent_pid = Pid::from_u32(pid);
    for (_, proc) in sys.processes() {
        if proc.parent() == Some(parent_pid) {
            let name = proc.name().to_string_lossy().to_lowercase();
            if name.contains("ssh") {
                let cmd_parts: Vec<String> = proc.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect();
                let cmd_line = cmd_parts.join(" ");
                let target = parse_ssh_target(&cmd_line);
                if !target.is_empty() {
                    return Ok(SshInfo {
                        is_ssh: true,
                        target,
                        remote_cwd: String::new(),
                        remote_os: String::new(),
                        remote_shell: String::new(),
                    });
                }
            }
        }
    }

    Ok(SshInfo {
        is_ssh: false,
        target: String::new(),
        remote_cwd: String::new(),
        remote_os: String::new(),
        remote_shell: String::new(),
    })
}

fn parse_ssh_target(cmd_line: &str) -> String {
    // Parse ssh command line to extract [user@]host
    // Skip flags like -p, -i, -o etc. (flags that take a value)
    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    let flags_with_args = [
        "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O",
        "-o", "-p", "-Q", "-R", "-S", "-W", "-w",
    ];
    let mut skip_next = false;
    let mut target = String::new();

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            continue; // skip "ssh"
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if flags_with_args.contains(part) {
            skip_next = true;
            continue;
        }
        if part.starts_with('-') {
            continue; // boolean flags like -v, -N, etc.
        }
        // First non-flag argument is the target
        target = part.to_string();
        break;
    }

    target
}

#[cfg(unix)]
fn ssh_multiplex_cmd(target: &str) -> std::process::Command {
    let socket_dir = std::env::temp_dir().join("drover-ssh");
    let _ = std::fs::create_dir_all(&socket_dir);
    let socket_path = socket_dir.join(format!("ctrl-{}", target.replace(['@', '.', ':'], "_")));

    let mut cmd = std::process::Command::new("ssh");
    cmd.args([
        "-o", "ControlMaster=auto",
        "-o", &format!("ControlPath={}", socket_path.display()),
        "-o", "ControlPersist=60",
        "-o", "ConnectTimeout=5",
        "-o", "BatchMode=yes",
        target,
    ]);
    cmd
}

#[cfg(unix)]
fn get_remote_cwd(target: &str) -> Result<String, String> {
    let script = r#"
pid=$(ps -eo pid,tty,comm --no-headers 2>/dev/null | awk '$2 ~ /pts/ && ($3 == "bash" || $3 == "zsh" || $3 == "sh" || $3 == "fish") {print $1}' | tail -1)
if [ -n "$pid" ] && [ -d "/proc/$pid" ]; then
  readlink /proc/$pid/cwd 2>/dev/null && exit 0
fi
pwd
"#;

    let output = ssh_multiplex_cmd(target)
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("Failed to get remote CWD".to_string())
    }
}

#[cfg(unix)]
fn get_remote_os(target: &str) -> Result<String, String> {
    let output = ssh_multiplex_cmd(target)
        .arg("uname -srm 2>/dev/null || echo unknown")
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("Failed to get remote OS".to_string())
    }
}

#[cfg(unix)]
fn get_remote_shell(target: &str) -> Result<String, String> {
    let output = ssh_multiplex_cmd(target)
        .arg("basename \"$SHELL\" 2>/dev/null || echo sh")
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("Failed to get remote shell".to_string())
    }
}
