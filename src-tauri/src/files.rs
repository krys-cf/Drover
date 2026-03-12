use serde::Serialize;

#[cfg(unix)]
fn ssh_command(ssh_target: &str) -> std::process::Command {
    let socket_dir = std::env::temp_dir().join("drover-ssh");
    let _ = std::fs::create_dir_all(&socket_dir);
    let socket_path = socket_dir.join(format!("ctrl-{}", ssh_target.replace(['@', '.', ':'], "_")));

    let mut cmd = std::process::Command::new("ssh");
    cmd.args([
        "-o", "ControlMaster=auto",
        "-o", &format!("ControlPath={}", socket_path.display()),
        "-o", "ControlPersist=60",
        "-o", "ConnectTimeout=5",
        "-o", "BatchMode=yes",
        ssh_target,
    ]);
    cmd
}

#[derive(Serialize, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub path: String,
}

/// Normalize a path to use forward slashes (for consistent frontend handling on all platforms)
fn normalize_separators(path: &str) -> String {
    path.replace('\\', "/")
}

fn expand_home(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            let expanded = path.replacen('~', &home.to_string_lossy().as_ref(), 1);
            // On Windows, the frontend uses forward slashes but the OS needs native separators
            return expanded.replace('/', std::path::MAIN_SEPARATOR_STR);
        }
    }
    #[cfg(windows)]
    {
        return path.replace('/', "\\");
    }
    #[allow(unreachable_code)]
    path.to_string()
}

fn replace_home_prefix(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path == home_str {
            return "~".to_string();
        }
        if let Some(rest) = path.strip_prefix(home_str.as_ref()) {
            return normalize_separators(&format!("~{rest}"));
        }
    }
    normalize_separators(path)
}

#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<DirEntry>, String> {
    let dir_path = expand_home(&path);

    let mut entries: Vec<DirEntry> = Vec::new();
    let read_dir =
        std::fs::read_dir(&dir_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let is_hidden = name.starts_with('.');
        let full_path = entry.path().to_string_lossy().to_string();

        let display_path = replace_home_prefix(&full_path);

        entries.push(DirEntry {
            name,
            is_dir: metadata.is_dir(),
            is_hidden,
            path: display_path,
        });
    }

    // Sort: directories first, then files, alphabetical within each group (case-insensitive)
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

#[tauri::command]
pub fn read_file_contents(path: String) -> Result<String, String> {
    let file_path = expand_home(&path);

    let metadata =
        std::fs::metadata(&file_path).map_err(|e| format!("Cannot read file: {}", e))?;

    // Limit to 10MB to avoid loading huge files
    if metadata.len() > 10 * 1024 * 1024 {
        return Err("File is too large (>10MB)".to_string());
    }

    std::fs::read_to_string(&file_path).map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub fn write_file_contents(path: String, contents: String) -> Result<(), String> {
    let file_path = expand_home(&path);
    std::fs::write(&file_path, contents).map_err(|e| format!("Failed to write file: {}", e))
}

#[tauri::command]
pub fn read_file_base64(path: String) -> Result<String, String> {
    use base64::Engine;
    let file_path = expand_home(&path);
    let metadata = std::fs::metadata(&file_path).map_err(|e| format!("Cannot read file: {}", e))?;
    if metadata.len() > 50 * 1024 * 1024 {
        return Err("File is too large (>50MB)".to_string());
    }
    let bytes = std::fs::read(&file_path).map_err(|e| format!("Failed to read file: {}", e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

#[cfg(unix)]
#[tauri::command]
pub fn read_remote_file_base64(ssh_target: String, path: String) -> Result<String, String> {
    let escaped = path.replace('\'', "'\\''");
    let remote_cmd = format!("base64 '{}'", escaped);
    let output = ssh_command(&ssh_target)
        .arg(&remote_cmd)
        .output()
        .map_err(|e| format!("SSH failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to read remote file: {}", stderr.trim()));
    }

    let b64 = String::from_utf8_lossy(&output.stdout).replace('\n', "").replace('\r', "");
    Ok(b64)
}

#[cfg(windows)]
#[tauri::command]
pub fn read_remote_file_base64(_ssh_target: String, _path: String) -> Result<String, String> {
    Err("SSH remote file access is not yet supported on Windows".to_string())
}

#[cfg(unix)]
#[tauri::command]
pub fn list_remote_directory(ssh_target: String, path: String) -> Result<Vec<DirEntry>, String> {
    let cmd = format!(
        "ls -1ap '{}'",
        path.replace('\'', "'\\''") 
    );

    let output = ssh_command(&ssh_target)
        .arg(&cmd)
        .output()
        .map_err(|e| format!("SSH failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Remote ls failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries: Vec<DirEntry> = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line == "./" || line == "../" {
            continue;
        }

        let is_dir = line.ends_with('/');
        let name = if is_dir {
            line.trim_end_matches('/').to_string()
        } else {
            line.to_string()
        };

        if name.is_empty() {
            continue;
        }

        let is_hidden = name.starts_with('.');
        let full_path = if path.ends_with('/') {
            format!("{}{}", path, &name)
        } else {
            format!("{}/{}", path, &name)
        };

        entries.push(DirEntry {
            name,
            is_dir,
            is_hidden,
            path: full_path,
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

#[cfg(windows)]
#[tauri::command]
pub fn list_remote_directory(_ssh_target: String, _path: String) -> Result<Vec<DirEntry>, String> {
    Err("SSH remote directory listing is not yet supported on Windows".to_string())
}

#[cfg(unix)]
#[tauri::command]
pub fn read_remote_file_contents(ssh_target: String, path: String) -> Result<String, String> {
    let size_cmd = format!(
        "stat -f%z '{}' 2>/dev/null || stat -c%s '{}' 2>/dev/null",
        path.replace('\'', "'\\''"),
        path.replace('\'', "'\\''") 
    );

    let size_output = ssh_command(&ssh_target)
        .arg(&size_cmd)
        .output()
        .map_err(|e| format!("SSH failed: {}", e))?;

    if size_output.status.success() {
        let size_str = String::from_utf8_lossy(&size_output.stdout).trim().to_string();
        if let Ok(size) = size_str.parse::<u64>() {
            if size > 10 * 1024 * 1024 {
                return Err("Remote file is too large (>10MB)".to_string());
            }
        }
    }

    let cat_cmd = format!("cat '{}'", path.replace('\'', "'\\''"));

    let output = ssh_command(&ssh_target)
        .arg(&cat_cmd)
        .output()
        .map_err(|e| format!("SSH failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to read remote file: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(windows)]
#[tauri::command]
pub fn read_remote_file_contents(_ssh_target: String, _path: String) -> Result<String, String> {
    Err("SSH remote file access is not yet supported on Windows".to_string())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
    let file_path = expand_home(&path);
    if std::path::Path::new(&file_path).exists() {
        return Err("File already exists".to_string());
    }
    if let Some(parent) = std::path::Path::new(&file_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dirs: {}", e))?;
    }
    std::fs::write(&file_path, "").map_err(|e| format!("Failed to create file: {}", e))
}

#[tauri::command]
pub fn create_directory(path: String) -> Result<(), String> {
    let dir_path = expand_home(&path);
    if std::path::Path::new(&dir_path).exists() {
        return Err("Directory already exists".to_string());
    }
    std::fs::create_dir_all(&dir_path).map_err(|e| format!("Failed to create directory: {}", e))
}

#[tauri::command]
pub fn rename_path(old_path: String, new_path: String) -> Result<(), String> {
    let src = expand_home(&old_path);
    let dst = expand_home(&new_path);
    if !std::path::Path::new(&src).exists() {
        return Err("Source path does not exist".to_string());
    }
    if std::path::Path::new(&dst).exists() {
        return Err("Destination already exists".to_string());
    }
    std::fs::rename(&src, &dst).map_err(|e| format!("Failed to rename: {}", e))
}

#[tauri::command]
pub fn delete_path(path: String) -> Result<(), String> {
    let p = expand_home(&path);
    let meta = std::fs::metadata(&p).map_err(|e| format!("Path not found: {}", e))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(&p).map_err(|e| format!("Failed to delete directory: {}", e))
    } else {
        std::fs::remove_file(&p).map_err(|e| format!("Failed to delete file: {}", e))
    }
}

#[tauri::command]
pub fn copy_path(source: String, dest: String) -> Result<(), String> {
    let src = expand_home(&source);
    let dst = expand_home(&dest);
    let meta = std::fs::metadata(&src).map_err(|e| format!("Source not found: {}", e))?;
    if meta.is_dir() {
        copy_dir_recursive(&src, &dst)
    } else {
        if let Some(parent) = std::path::Path::new(&dst).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dirs: {}", e))?;
        }
        std::fs::copy(&src, &dst).map_err(|e| format!("Failed to copy file: {}", e))?;
        Ok(())
    }
}

fn copy_dir_recursive(src: &str, dst: &str) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("Failed to create directory: {}", e))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("Failed to read directory: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = std::path::Path::new(dst).join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(
                &src_path.to_string_lossy(),
                &dst_path.to_string_lossy(),
            )?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed to copy {}: {}", src_path.display(), e))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
#[tauri::command]
pub fn write_remote_file_contents(ssh_target: String, path: String, contents: String) -> Result<(), String> {
    let escaped_path = path.replace('\'', "'\\''");
    let remote_cmd = format!("cat > '{}'", escaped_path);

    let mut child = ssh_command(&ssh_target)
        .arg(&remote_cmd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("SSH failed: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(contents.as_bytes())
            .map_err(|e| format!("Failed to write to remote: {}", e))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("SSH failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to write remote file: {}", stderr.trim()));
    }

    Ok(())
}

#[cfg(windows)]
#[tauri::command]
pub fn write_remote_file_contents(_ssh_target: String, _path: String, _contents: String) -> Result<(), String> {
    Err("SSH remote file writing is not yet supported on Windows".to_string())
}
