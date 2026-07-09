use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

const SETTINGS_FILE_NAME: &str = "user-settings.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserSettings {
    pub dark_theme: bool,
    pub enter_to_send: bool,
    pub show_timestamps: bool,
    pub auto_join_voice: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            dark_theme: true,
            enter_to_send: true,
            show_timestamps: false,
            auto_join_voice: false,
        }
    }
}

pub fn load_or_create_user_settings(data_dir: impl AsRef<Path>) -> io::Result<UserSettings> {
    let data_dir = data_dir.as_ref();
    fs::create_dir_all(data_dir)?;

    let path = data_dir.join(SETTINGS_FILE_NAME);
    if path.exists() {
        let raw = fs::read_to_string(path)?;
        return serde_json::from_str(&raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
    }

    let settings = UserSettings::default();
    save_user_settings(data_dir, &settings)?;
    Ok(settings)
}

pub fn save_user_settings(data_dir: impl AsRef<Path>, settings: &UserSettings) -> io::Result<()> {
    let data_dir = data_dir.as_ref();
    fs::create_dir_all(data_dir)?;

    let path = data_dir.join(SETTINGS_FILE_NAME);
    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    if path.exists() {
        fs::write(path, serialized)?;
    } else {
        write_secure_file(&path, &serialized)?;
    }

    Ok(())
}

fn write_secure_file(path: &Path, content: &str) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        fs::write(path, content)?;
        Ok(())
    }
}
