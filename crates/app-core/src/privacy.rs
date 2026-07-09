use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum PrivacyMode {
    Direct,
    #[default]
    Balanced,
    Anonymous,
}

impl fmt::Display for PrivacyMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Direct => "Direct",
            Self::Balanced => "Balanced",
            Self::Anonymous => "Anonymous",
        };

        formatter.write_str(label)
    }
}
