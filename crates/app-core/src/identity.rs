use std::fmt;

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use x25519_dalek::StaticSecret;

use crate::privacy::PrivacyMode;

const PUBLIC_KEY_BYTES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IdentityId(String);

impl IdentityId {
    pub fn from_public_key(public_key: [u8; PUBLIC_KEY_BYTES]) -> Self {
        Self(hex_encode(&public_key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn short(&self) -> &str {
        let short_len = 12;
        self.0.get(..short_len).unwrap_or(self.0.as_str())
    }
}

impl fmt::Display for IdentityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicIdentity {
    bytes: [u8; PUBLIC_KEY_BYTES],
    id: IdentityId,
}

impl PublicIdentity {
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_BYTES]) -> Self {
        let id = IdentityId::from_public_key(bytes);
        Self { bytes, id }
    }

    pub fn bytes(&self) -> &[u8; PUBLIC_KEY_BYTES] {
        &self.bytes
    }

    pub fn id(&self) -> &IdentityId {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceKey {
    pub label: String,
    pub public_key: [u8; PUBLIC_KEY_BYTES],
}

impl DeviceKey {
    pub fn new(label: impl Into<String>, public_key: [u8; PUBLIC_KEY_BYTES]) -> Self {
        Self {
            label: label.into(),
            public_key,
        }
    }

    pub fn fingerprint(&self) -> String {
        hex_encode(&self.public_key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalIdentity {
    pub display_name: String,
    pub privacy_mode: PrivacyMode,
    pub public_identity: PublicIdentity,
    pub devices: Vec<DeviceKey>,
}

impl LocalIdentity {
    pub fn new(
        display_name: impl Into<String>,
        privacy_mode: PrivacyMode,
        public_identity: PublicIdentity,
        devices: Vec<DeviceKey>,
    ) -> Self {
        Self {
            display_name: display_name.into(),
            privacy_mode,
            public_identity,
            devices,
        }
    }

    pub fn identity_id(&self) -> &IdentityId {
        self.public_identity.id()
    }
}

pub fn generate_ed25519_secret() -> [u8; PUBLIC_KEY_BYTES] {
    SigningKey::generate(&mut OsRng).to_bytes()
}

pub fn ed25519_public_from_secret(secret: [u8; PUBLIC_KEY_BYTES]) -> [u8; PUBLIC_KEY_BYTES] {
    SigningKey::from_bytes(&secret).verifying_key().to_bytes()
}

pub fn generate_x25519_secret() -> [u8; PUBLIC_KEY_BYTES] {
    StaticSecret::random_from_rng(OsRng).to_bytes()
}

pub fn x25519_public_from_secret(secret: [u8; PUBLIC_KEY_BYTES]) -> [u8; PUBLIC_KEY_BYTES] {
    let secret = StaticSecret::from(secret);
    x25519_dalek::PublicKey::from(&secret).to_bytes()
}

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        use std::fmt::Write;

        let _ = write!(&mut output, "{byte:02x}");
    }

    output
}

pub fn hex_decode<const N: usize>(value: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2 {
        return Err(format!(
            "invalid hex length: expected {}, got {}",
            N * 2,
            value.len()
        ));
    }

    let mut output = [0_u8; N];
    let bytes = value.as_bytes();

    for index in 0..N {
        let left = decode_hex_nibble(bytes[index * 2])?;
        let right = decode_hex_nibble(bytes[index * 2 + 1])?;
        output[index] = (left << 4) | right;
    }

    Ok(output)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex character: {}", byte as char)),
    }
}

#[cfg(test)]
mod tests {
    use super::{hex_decode, hex_encode, IdentityId};

    #[test]
    fn identity_id_is_hex_encoded() {
        let identity_id = IdentityId::from_public_key([0xAB; 32]);

        assert_eq!(identity_id.as_str().len(), 64);
        assert!(identity_id.as_str().starts_with("abab"));
    }

    #[test]
    fn hex_round_trip_works() {
        let bytes = [0x42; 32];
        let encoded = hex_encode(&bytes);
        let decoded = hex_decode::<32>(&encoded).expect("hex decode should work");

        assert_eq!(decoded, bytes);
    }
}
