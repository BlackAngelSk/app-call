//! Self-update module: checks GitHub Releases for a newer version and replaces the running binary.
//!
//! Usage:
//!   - `--check-update` — prints whether a newer version is available and exits.
//!   - `--update`       — downloads and installs the update, then restarts.
//!   - On normal startup the app performs a **silent** background check and prints a
//!     hint to stderr if an update is available (unless `APP_CALL_NO_UPDATE_CHECK=1`).

use std::env;
use std::io::Write;
use std::process;

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// GitHub repository that hosts releases.
const REPO: &str = "BlackAngelSk/app-call";

/// Current application version from Cargo.
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── GitHub API types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Result of an update check.
#[derive(Debug)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub download_url: Option<String>,
    pub checksum_url: Option<String>,
    pub asset_size: u64,
}

/// Process command-line update flags.
///
/// Returns `true` if the process should continue running (no update action taken),
/// or `false` if the process should exit (update handled or error).
pub fn handle_update_flags() -> bool {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--check-update") {
        match check_for_update() {
            Ok(info) => {
                if info.current == info.latest {
                    println!("Already running the latest version (v{}).", info.current);
                } else {
                    println!("Update available: v{} -> v{}", info.current, info.latest);
                    println!("Run the app with --update to install it.");
                }
            }
            Err(e) => {
                eprintln!("Update check failed: {e}");
            }
        }
        return false; // exit
    }

    if args.iter().any(|a| a == "--update") {
        match perform_update() {
            Ok(updated) => {
                if updated {
                    println!("Update complete. Restarting...");
                    restart_process();
                }
                // If not updated (already latest), just exit.
            }
            Err(e) => {
                eprintln!("Update failed: {e}");
                process::exit(1);
            }
        }
        return false;
    }

    true // no update flag, continue normally
}

/// Silent startup check: prints a hint to stderr if a newer version exists.
/// Honours `APP_CALL_NO_UPDATE_CHECK=1` to disable.
pub fn try_background_check() {
    if env::var("APP_CALL_NO_UPDATE_CHECK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return;
    }

    // Run in a blocking OS thread so we don't interfere with the caller's async runtime.
    let handle = std::thread::spawn(|| match check_for_update() {
        Ok(info) => {
            if info.current != info.latest {
                eprintln!(
                    "\n[app-call] A new version is available: v{} -> v{}\
                     \n           Download: https://github.com/{REPO}/releases/latest\
                     \n           Or run with --update to install automatically.\n",
                    info.current, info.latest,
                );
            }
        }
        Err(e) => {
            // Silently ignore network errors during background check.
            eprintln!("[app-call] Background update check skipped: {e}");
        }
    });

    // Don't block the caller; the thread will print on its own.
    let _ = handle.join();
}

// ── Core logic ──────────────────────────────────────────────────────────────

/// Fetch the latest release from GitHub and compare with the running version.
fn check_for_update() -> Result<UpdateInfo, Box<dyn std::error::Error>> {
    let current = Version::parse(PKG_VERSION)?;
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("app-call/{PKG_VERSION}"))
        .build()?;

    let release: GhRelease = client.get(&url).send()?.error_for_status()?.json()?;

    let tag = release.tag_name.trim_start_matches('v');
    let latest = Version::parse(tag)?;

    let target = target_triple();

    // Look for an asset matching our platform.
    let asset_name = format!("app-call-v{latest}-{target}");
    let archive_name = if cfg!(target_os = "windows") {
        format!("{asset_name}.zip")
    } else {
        format!("{asset_name}.tar.gz")
    };
    let checksum_name = format!("{archive_name}.sha256");

    let download_url = release
        .assets
        .iter()
        .find(|a| a.name == archive_name)
        .map(|a| a.browser_download_url.clone());

    let checksum_url = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .map(|a| a.browser_download_url.clone());

    let asset_size = release
        .assets
        .iter()
        .find(|a| a.name == archive_name)
        .map(|a| a.size)
        .unwrap_or(0);

    Ok(UpdateInfo {
        current: current.to_string(),
        latest: latest.to_string(),
        download_url,
        checksum_url,
        asset_size,
    })
}

/// Download the latest release and replace the running binary.
/// Returns `Ok(true)` if an update was installed, `Ok(false)` if already up to date.
fn perform_update() -> Result<bool, Box<dyn std::error::Error>> {
    let info = check_for_update()?;

    if info.current == info.latest {
        println!("Already running the latest version (v{}).", info.current);
        return Ok(false);
    }

    let download_url = info
        .download_url
        .ok_or_else(|| "No matching release asset found for this platform.".to_string())?;
    let checksum_url = info
        .checksum_url
        .ok_or_else(|| "No checksum file found for this release asset.".to_string())?;

    println!("Downloading v{} ({:.1} MiB)...", info.latest, info.asset_size as f64 / 1048576.0);

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("app-call/{}", info.current))
        .build()?;

    // Download checksum.
    let expected_checksum = client
        .get(&checksum_url)
        .send()?
        .error_for_status()?
        .text()?;
    let expected_checksum = expected_checksum
        .split_whitespace()
        .next()
        .ok_or("Empty checksum file")?
        .to_lowercase();

    // Download archive.
    let archive_bytes = client
        .get(&download_url)
        .send()?
        .error_for_status()?
        .bytes()?;

    // Verify checksum.
    let mut hasher = Sha256::new();
    hasher.update(&archive_bytes);
    let actual_checksum = format!("{:x}", hasher.finalize());

    if actual_checksum != expected_checksum {
        return Err(format!(
            "Checksum mismatch!\n  expected: {expected_checksum}\n  actual:   {actual_checksum}"
        )
        .into());
    }
    println!("Checksum verified.");

    // Extract the binary from the archive.
    let new_binary = extract_binary(&archive_bytes)?;

    // Write the new binary to a temporary file.
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("app-call-update.tmp");
    let mut tmp_file = std::fs::File::create(&tmp_path)?;
    tmp_file.write_all(&new_binary)?;
    drop(tmp_file);

    // Replace the running binary.
    let current_exe = env::current_exe()?;
    println!("Replacing {}...", current_exe.display());
    self_replace::self_replace(&tmp_path)?;
    // Clean up temp file.
    let _ = std::fs::remove_file(&tmp_path);
    println!("Binary replaced successfully.");

    Ok(true)
}

/// Extract the platform binary from a `.tar.gz` or `.zip` archive.
fn extract_binary(archive: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // The archive should contain exactly one executable file named `app-call` (or
    // `app-call.exe` on Windows). We locate it by name.

    let binary_name = if cfg!(target_os = "windows") {
        "app-call.exe"
    } else {
        "app-call"
    };

    #[cfg(not(target_os = "windows"))]
    {
        use flate2::read::GzDecoder;
        use std::io::Read;
        let decoder = GzDecoder::new(archive);
        let mut tar = tar::Archive::new(decoder);
        for entry in tar.entries()? {
            let mut entry = entry?;
            let path = entry.header().path()?;
            if path
                .file_name()
                .map(|n| n == binary_name)
                .unwrap_or(false)
            {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                return Ok(buf);
            }
        }
        Err(format!("Binary '{binary_name}' not found in archive").into())
    }

    #[cfg(target_os = "windows")]
    {
        let reader = std::io::Cursor::new(archive);
        let mut zip = zip::ZipArchive::new(reader)?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            if file
                .name()
                .ends_with(binary_name)
            {
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut buf)?;
                return Ok(buf);
            }
        }
        Err(format!("Binary '{binary_name}' not found in archive").into())
    }
}

/// Restart the current process with the same arguments.
fn restart_process() {
    let exe = match env::current_exe() {
        Ok(e) => e,
        Err(_) => {
            eprintln!("Could not determine executable path for restart.");
            process::exit(0);
        }
    };
    let args: Vec<String> = env::args().skip(1).collect();
    let _ = process::Command::new(exe).args(&args).spawn();
    process::exit(0);
}

/// Return the Rust target triple for the current compilation.
const fn target_triple() -> &'static str {
    // We use the cfg values at compile time rather than a runtime lookup.
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "aarch64") {
        "aarch64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "unknown-unknown"
    }
}