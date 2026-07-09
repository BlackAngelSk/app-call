use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::identity::{
    ed25519_public_from_secret, generate_ed25519_secret, generate_x25519_secret, hex_decode,
    hex_encode, x25519_public_from_secret, DeviceKey, LocalIdentity, PublicIdentity,
};
use crate::model::AppModel;
use crate::privacy::PrivacyMode;
use crate::seed::demo_app;

const IDENTITY_FILE_NAME: &str = "identity.json";
const BYTES_32: usize = 32;

#[derive(Debug, Deserialize, Serialize)]
struct StoredIdentity {
    version: u8,
    display_name: String,
    privacy_mode: PrivacyMode,
    root_signing_secret_hex: String,
    root_exchange_secret_hex: String,
    device_label: String,
    device_signing_secret_hex: String,
}

pub fn load_or_create_app(data_dir: impl AsRef<Path>) -> io::Result<AppModel> {
    let data_dir = data_dir.as_ref();
    fs::create_dir_all(data_dir)?;

    let identity_path = data_dir.join(IDENTITY_FILE_NAME);

    let local_identity = if identity_path.exists() {
        let raw = fs::read_to_string(&identity_path)?;
        let stored: StoredIdentity = serde_json::from_str(&raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        stored.into_local_identity()?
    } else {
        let stored = StoredIdentity::new_default();
        let serialized = serde_json::to_string_pretty(&stored)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_identity_file(&identity_path, &serialized)?;
        stored.into_local_identity()?
    };

    let mut app = demo_app();
    app.local_identity = local_identity;
    Ok(app)
}

pub fn update_display_name(data_dir: impl AsRef<Path>, display_name: &str) -> io::Result<()> {
    let sanitized = display_name.trim();
    if sanitized.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "display name cannot be empty",
        ));
    }

    let data_dir = data_dir.as_ref();
    fs::create_dir_all(data_dir)?;

    let identity_path = data_dir.join(IDENTITY_FILE_NAME);

    let mut stored = if identity_path.exists() {
        let raw = fs::read_to_string(&identity_path)?;
        serde_json::from_str::<StoredIdentity>(&raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
    } else {
        StoredIdentity::new_default()
    };

    stored.display_name = sanitized.to_string();
    let serialized = serde_json::to_string_pretty(&stored)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    if identity_path.exists() {
        fs::write(identity_path, serialized)?;
    } else {
        write_identity_file(&identity_path, &serialized)?;
    }

    Ok(())
}

fn write_identity_file(path: &Path, content: &str) -> io::Result<()> {
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

impl StoredIdentity {
    fn new_default() -> Self {
        Self {
            version: 1,
            display_name: "local-user".to_string(),
            privacy_mode: PrivacyMode::Balanced,
            root_signing_secret_hex: hex_encode(&generate_ed25519_secret()),
            root_exchange_secret_hex: hex_encode(&generate_x25519_secret()),
            device_label: "primary-device".to_string(),
            device_signing_secret_hex: hex_encode(&generate_ed25519_secret()),
        }
    }

    fn into_local_identity(self) -> io::Result<LocalIdentity> {
        let root_signing_secret =
            parse_hex::<BYTES_32>(&self.root_signing_secret_hex, "root_signing_secret_hex")?;
        let root_exchange_secret =
            parse_hex::<BYTES_32>(&self.root_exchange_secret_hex, "root_exchange_secret_hex")?;
        let device_signing_secret =
            parse_hex::<BYTES_32>(&self.device_signing_secret_hex, "device_signing_secret_hex")?;

        let public_identity = PublicIdentity::from_bytes(ed25519_public_from_secret(root_signing_secret));

        // Exchange key derivation is performed here to verify persisted key material shape.
        let _root_exchange_public = x25519_public_from_secret(root_exchange_secret);

        let device = DeviceKey::new(
            self.device_label,
            ed25519_public_from_secret(device_signing_secret),
        );

        Ok(LocalIdentity::new(
            self.display_name,
            self.privacy_mode,
            public_identity,
            vec![device],
        ))
    }
}

fn parse_hex<const N: usize>(value: &str, field_name: &str) -> io::Result<[u8; N]> {
    hex_decode::<N>(value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid {}: {}", field_name, error),
        )
    })
}
