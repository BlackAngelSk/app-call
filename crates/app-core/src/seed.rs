use crate::identity::{DeviceKey, LocalIdentity, PublicIdentity};
use crate::model::{AppModel, ChannelKind, ChannelSummary, SpaceSummary};
use crate::privacy::PrivacyMode;

pub fn demo_app() -> AppModel {
    let local_identity = LocalIdentity::new(
        "local-user",
        PrivacyMode::Balanced,
        PublicIdentity::from_bytes([0x42; 32]),
        vec![
            DeviceKey::new("linux-desktop", [0x11; 32]),
            DeviceKey::new("windows-laptop", [0x22; 32]),
        ],
    );

    let spaces = vec![
        SpaceSummary::new(
            "Core Protocol",
            8,
            vec![
                ChannelSummary::new("architecture", ChannelKind::Text, true),
                ChannelSummary::new("relay-lab", ChannelKind::Voice, true),
                ChannelSummary::new("media-tests", ChannelKind::Media, true),
            ],
        ),
        SpaceSummary::new(
            "Launch Room",
            3,
            vec![
                ChannelSummary::new("announcements", ChannelKind::Announcement, true),
                ChannelSummary::new("general", ChannelKind::Text, true),
            ],
        ),
    ];

    AppModel {
        local_identity,
        spaces,
    }
}
