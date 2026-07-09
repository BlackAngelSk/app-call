use serde::{Deserialize, Serialize};

use crate::identity::LocalIdentity;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppModel {
    pub local_identity: LocalIdentity,
    pub spaces: Vec<SpaceSummary>,
}

impl AppModel {
    pub fn total_channels(&self) -> usize {
        self.spaces.iter().map(|space| space.channels.len()).sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceSummary {
    pub name: String,
    pub member_count: u32,
    pub channels: Vec<ChannelSummary>,
}

impl SpaceSummary {
    pub fn new(
        name: impl Into<String>,
        member_count: u32,
        channels: Vec<ChannelSummary>,
    ) -> Self {
        Self {
            name: name.into(),
            member_count,
            channels,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelSummary {
    pub name: String,
    pub kind: ChannelKind,
    pub encrypted: bool,
}

impl ChannelSummary {
    pub fn new(name: impl Into<String>, kind: ChannelKind, encrypted: bool) -> Self {
        Self {
            name: name.into(),
            kind,
            encrypted,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKind {
    Text,
    Voice,
    Media,
    Announcement,
}

impl ChannelKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Voice => "voice",
            Self::Media => "media",
            Self::Announcement => "announcement",
        }
    }
}

// ── Wire protocol messages ──────────────────────────────────────────────────

/// Messages exchanged between peers over TCP.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProtocolMessage {
    /// First message sent on a new connection.
    Handshake {
        node_id: String,
        nickname: String,
        listen_port: u16,
    },
    /// Share known peer addresses with the remote side.
    PeerExchange {
        peers: Vec<PeerAddress>,
    },
    /// A chat message from a user.
    Chat {
        id: String,
        from_node: String,
        from_name: String,
        body: String,
        timestamp: u64,
    },
    /// Keep-alive ping.
    Ping {
        ts: u64,
    },
    /// Keep-alive pong response.
    Pong {
        ts: u64,
    },
}

/// A reachable peer address shared during peer exchange.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerAddress {
    pub node_id: String,
    pub nickname: String,
    pub addr: String, // "ip:port"
}