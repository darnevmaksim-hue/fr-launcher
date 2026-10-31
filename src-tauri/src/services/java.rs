use crate::models::JavaInstallation;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn detect_all() -> Vec<JavaInstallation> {
    let mut found: Vec<JavaInstallation> = Vec::new();

    for candidate in candidate_paths() {
        if !candidate.is_file() {
            continue;
        }
        if found
            .iter()
            .any(|j| same_path(&j.path, &candidate.to_string_lossy()))
        {
            continue;
        }
        if let Some(install) = probe_java(&candidate) {
            log::info!("java: found {} (major {})", install.path, install.version);
            found.push(install);
        }
    }

    found.sort_by(|a, b| b.version.cmp(&a.version));
    log::info!(
        "java: {} installation(s) detected: {}",
        found.len(),
        found
            .iter()
            .map(|j| format!("{} ({})", j.path, j.version))
            .collect::<Vec<_>>()
            .join("; ")
    );
    found
}

fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

fn java_exe() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut roots: Vec<(PathBuf, usize)> = Vec::new();

    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        if !java_home.trim().is_empty() {
            paths.push(PathBuf::from(java_home).join("bin").join(java_exe()));
        }
    }

    if let Some(home) = dirs::home_dir() {
        // JDK, которые скачивает наш лаунчер (~/.fr-launcher/jdk/jdk-17...)
        roots.push((home.join(".fr-launcher").join("jdk"), 2));
        roots.push((home.join(".jdks"), 2));
        roots.push((home.join(".gradle").join("jdks"), 2));
        roots.push((home.join(".sdkman").join("candidates").join("java"), 2));
    }

    #[cfg(windows)]
    {
        let mut vendor_roots: Vec<PathBuf> = Vec::new();
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Ok(base) = std::env::var(var) {
                let base = PathBuf::from(base);
                for vendor in [
                    "Java",
                    "Eclipse Adoptium",
                    "Eclipse Foundation",
                    "Microsoft",
                    "Zulu",
                    "BellSoft",
                    "Amazon Corretto",
                    "GraalVM",
                    "JetBrains",
                    "Semeru",
                    "AdoptOpenJDK",
                    "RedHat",
                ] {
                    vendor_roots.push(base.join(vendor));
                }
            }
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            for vendor in ["Eclipse Adoptium", "Microsoft", "Zulu", "BellSoft"] {
                vendor_roots.push(local.join("Programs").join(vendor));
            }
            // Официальные рантаймы Minecraft
            roots.push((local.join("Packages"), 0));
        }
        for root in vendor_roots {
            roots.push((root, 2));
        }

        if let Ok(appdata) = std::env::var("APPDATA") {
            let appdata = PathBuf::from(appdata);
            roots.push((appdata.join(".minecraft").join("runtime"), 4));
        }
        if let Some(home) = dirs::home_dir() {
            roots.push((home.join("scoop").join("apps"), 3));
        }
    }

    #[cfg(not(windows))]
    {
        roots.push((PathBuf::from("/usr/lib/jvm"), 2));
        roots.push((PathBuf::from("/usr/local/lib/jvm"), 2));
        roots.push((PathBuf::from("/opt/java"), 2));
    }

    for (root, depth) in roots {
        if depth > 0 {
            scan_dir(&root, depth, &mut paths);
        }
    }

    // java из PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(java_exe());
            if candidate.is_file() {
                paths.push(candidate);
            }
        }
    }

    // Системный поиск
    #[cfg(windows)]
    {
        if let Ok(output) = Command::new("where").arg("java").output() {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        paths.push(PathBuf::from(line));
                    }
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(output) = Command::new("which").arg("java").output() {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() {
                    paths.push(PathBuf::from(s));
                }
            }
        }
    }

    paths
}

fn scan_dir(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 || !root.is_dir() {
        return;
    }
    let direct = root.join("bin").join(java_exe());
    if direct.is_file() {
        out.push(direct);
        return;
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, depth - 1, out);
            }
        }
    }
}

fn probe_java(path: &Path) -> Option<JavaInstallation> {
    if !path.exists() {
        return None;
    }
    let output = Command::new(path).arg("-version").output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stderr.trim().is_empty() { stdout } else { stderr };
    let version = parse_java_version(&text)?;
    let vendor = text.lines().next().unwrap_or("Java").to_string();
    Some(JavaInstallation {
        path: path.to_string_lossy().to_string(),
        version,
        vendor,
    })
}

fn parse_java_version(output: &str) -> Option<u8> {
    for line in output.lines() {
        if let Some(idx) = line.find("version \"") {
            let ver = &line[idx + 9..];
            let major = ver
                .split('.')
                .next()?
                .trim_end_matches('"')
                .parse::<u16>()
                .ok()?;
            let major = if major == 1 {
                ver.split('.').nth(1)?.parse().ok()?
            } else {
                major
            };
            return Some(major as u8);
        }
    }
    None
}
