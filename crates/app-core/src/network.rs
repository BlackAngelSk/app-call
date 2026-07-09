use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex};

use crate::model::{PeerAddress, ProtocolMessage};

// ── Public API ──────────────────────────────────────────────────────────────

/// An incoming chat message ready for display.
#[derive(Clone, Debug)]
pub struct IncomingChat {
    pub from_name: String,
    pub from_node: String,
    pub body: String,
    pub timestamp: u64,
}

/// An event surfaced to the terminal UI.
#[derive(Clone, Debug)]
pub enum NetworkEvent {
    PeerConnected {
        node_id: String,
        nickname: String,
        addr: String,
    },
    PeerDisconnected {
        node_id: String,
        nickname: String,
    },
    ChatReceived(IncomingChat),
    Info(String),
    Error(String),
}

/// Shared state for the networking layer.
pub struct NetworkState {
    pub node_id: String,
    pub nickname: String,
    pub listen_port: u16,
    peers: Arc<Mutex<HashMap<String, ConnectedPeer>>>,
    event_tx: broadcast::Sender<NetworkEvent>,
    seen_ids: Arc<Mutex<HashMap<String, ()>>>,
}

struct ConnectedPeer {
    info: PeerAddress,
    writer_tx: mpsc::Sender<ProtocolMessage>,
}

impl NetworkState {
    pub fn new(
        node_id: String,
        nickname: String,
        listen_port: u16,
    ) -> (Self, broadcast::Receiver<NetworkEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        let state = Self {
            node_id,
            nickname,
            listen_port,
            peers: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            seen_ids: Arc::new(Mutex::new(HashMap::new())),
        };
        (state, event_rx)
    }

    /// Start the TCP listener on 0.0.0.0:<port>.
    pub async fn start_listener(self: &Arc<Self>) -> io::Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.listen_port));
        let listener = TcpListener::bind(addr).await?;
        let _ = self.event_tx.send(NetworkEvent::Info(format!(
            "Listening on 0.0.0.0:{}",
            self.listen_port
        )));

        let state = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            if let Err(e) = accept_incoming(state, stream, peer_addr).await {
                                eprintln!("[net] incoming connection error from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("[net] accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Connect to a remote peer at the given address.
    pub async fn connect_peer(self: &Arc<Self>, addr: &str) -> io::Result<()> {
        let stream = TcpStream::connect(addr).await?;
        let peer_addr: SocketAddr = stream.peer_addr()?;

        let state = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = dial_outgoing(state, stream, peer_addr).await {
                eprintln!("[net] outgoing connection error to {}: {}", peer_addr, e);
            }
        });
        Ok(())
    }

    /// Broadcast a chat message to all connected peers.
    pub async fn send_chat(self: &Arc<Self>, body: &str) {
        let ts = now_ts();
        let id = uuid::Uuid::new_v4().to_string();
        let msg = ProtocolMessage::Chat {
            id: id.clone(),
            from_node: self.node_id.clone(),
            from_name: self.nickname.clone(),
            body: body.to_string(),
            timestamp: ts,
        };

        self.seen_ids.lock().await.insert(id, ());

        let peers = self.peers.lock().await;
        for (_pid, peer) in peers.iter() {
            let _ = peer.writer_tx.send(msg.clone()).await;
        }
    }

    /// Return a snapshot of connected peers.
    pub async fn list_peers(self: &Arc<Self>) -> Vec<PeerAddress> {
        let peers = self.peers.lock().await;
        peers.values().map(|p| p.info.clone()).collect()
    }

    /// Subscribe to network events.
    pub fn subscribe(&self) -> broadcast::Receiver<NetworkEvent> {
        self.event_tx.subscribe()
    }

    async fn add_peer(&self, info: PeerAddress, writer_tx: mpsc::Sender<ProtocolMessage>) {
        let _ = self.event_tx.send(NetworkEvent::PeerConnected {
            node_id: info.node_id.clone(),
            nickname: info.nickname.clone(),
            addr: info.addr.clone(),
        });
        self.peers.lock().await.insert(info.node_id.clone(), ConnectedPeer { info, writer_tx });
    }

    async fn remove_peer(&self, node_id: &str) {
        if let Some(peer) = self.peers.lock().await.remove(node_id) {
            let _ = self.event_tx.send(NetworkEvent::PeerDisconnected {
                node_id: node_id.to_string(),
                nickname: peer.info.nickname,
            });
        }
    }

    async fn mark_seen(&self, id: &str) -> bool {
        let mut seen = self.seen_ids.lock().await;
        if seen.contains_key(id) {
            false
        } else {
            seen.insert(id.to_string(), ());
            true
        }
    }

    async fn relay_chat(&self, msg: &ProtocolMessage, except_node: &str) {
        let peers = self.peers.lock().await;
        for (pid, peer) in peers.iter() {
            if pid != except_node {
                let _ = peer.writer_tx.send(msg.clone()).await;
            }
        }
    }
}

// ── Connection entry points ─────────────────────────────────────────────────

/// Accept an incoming TCP connection, perform handshake, then spawn read loop.
async fn accept_incoming(
    state: Arc<NetworkState>,
    stream: TcpStream,
    peer_addr: SocketAddr,
) -> io::Result<()> {
    let (reader, writer) = stream.into_split();
    let (write_tx, write_rx) = mpsc::channel::<ProtocolMessage>(64);
    tokio::spawn(writer_task(writer, write_rx));

    // Send our handshake immediately.
    send_msg(&write_tx, &make_handshake(&state)).await;

    // Read the remote handshake.
    let mut buf = BufReader::new(reader);
    let first_line = read_line_trimmed(&mut buf).await?;
    let handshake: ProtocolMessage = serde_json::from_str(&first_line)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let (remote_id, remote_nick, remote_port) = match handshake {
        ProtocolMessage::Handshake { node_id, nickname, listen_port } => (node_id, nickname, listen_port),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "expected handshake")),
    };

    let external_addr = format!("{}:{}", peer_addr.ip(), remote_port);
    let peer_info = PeerAddress {
        node_id: remote_id.clone(),
        nickname: remote_nick.clone(),
        addr: external_addr.clone(),
    };
    state.add_peer(peer_info, write_tx.clone()).await;

    // Share known peers.
    send_peer_exchange(&state, &write_tx, &remote_id).await;

    // Spawn the read loop with owned data.
    spawn_read_loop(state, buf, write_tx, remote_id);

    Ok(())
}

/// Handle an outgoing TCP connection: connect, handshake, spawn read loop.
async fn dial_outgoing(
    state: Arc<NetworkState>,
    stream: TcpStream,
    peer_addr: SocketAddr,
) -> io::Result<()> {
    let (reader, writer) = stream.into_split();
    let (write_tx, write_rx) = mpsc::channel::<ProtocolMessage>(64);
    tokio::spawn(writer_task(writer, write_rx));

    // Send our handshake.
    send_msg(&write_tx, &make_handshake(&state)).await;

    // Read the remote handshake.
    let mut buf = BufReader::new(reader);
    let first_line = read_line_trimmed(&mut buf).await?;
    let handshake: ProtocolMessage = serde_json::from_str(&first_line)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let (remote_id, remote_nick) = match handshake {
        ProtocolMessage::Handshake { node_id, nickname, listen_port: _ } => (node_id, nickname),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "expected handshake")),
    };

    let peer_info = PeerAddress {
        node_id: remote_id.clone(),
        nickname: remote_nick.clone(),
        addr: peer_addr.to_string(),
    };
    state.add_peer(peer_info, write_tx.clone()).await;

    // Share known peers.
    send_peer_exchange(&state, &write_tx, &remote_id).await;

    // Spawn the read loop with owned data.
    spawn_read_loop(state, buf, write_tx, remote_id);

    Ok(())
}

// ── Read loop (runs as its own spawned task with all owned data) ─────────────

fn spawn_read_loop(
    state: Arc<NetworkState>,
    mut buf: BufReader<tokio::net::tcp::OwnedReadHalf>,
    write_tx: mpsc::Sender<ProtocolMessage>,
    remote_id: String,
) {
    tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            let n = match buf.read_line(&mut line).await {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("[net] read error from {}: {}", remote_id, e);
                    break;
                }
            };
            if n == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let msg: ProtocolMessage = match serde_json::from_str(trimmed) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[net] bad message from {}: {}", remote_id, e);
                    continue;
                }
            };

            match msg {
                ProtocolMessage::Chat { id, from_node, from_name, body, timestamp } => {
                    if state.mark_seen(&id).await {
                        let _ = state.event_tx.send(NetworkEvent::ChatReceived(IncomingChat {
                            from_name: from_name.clone(),
                            from_node: from_node.clone(),
                            body: body.clone(),
                            timestamp,
                        }));
                        let relay = ProtocolMessage::Chat { id, from_node, from_name, body, timestamp };
                        state.relay_chat(&relay, &remote_id).await;
                    }
                }
                ProtocolMessage::PeerExchange { peers } => {
                    for addr in &peers {
                        if addr.node_id == state.node_id {
                            continue;
                        }
                        let already = state.peers.lock().await.contains_key(&addr.node_id);
                        if already {
                            continue;
                        }
                        let state2 = Arc::clone(&state);
                        let target = addr.addr.clone();
                        let nick = addr.nickname.clone();
                        let target_for_spawn = target.clone();
                        tokio::spawn(async move {
                            if let Err(e) = state2.connect_peer(&target_for_spawn).await {
                                eprintln!("[net] auto-connect to {} failed: {}", target_for_spawn, e);
                            }
                        });
                        let _ = state.event_tx.send(NetworkEvent::Info(format!(
                            "Peer exchange: learning about {} ({})", nick, target
                        )));
                    }
                }
                ProtocolMessage::Ping { ts } => {
                    let _ = write_tx.send(ProtocolMessage::Pong { ts }).await;
                }
                ProtocolMessage::Pong { .. } | ProtocolMessage::Handshake { .. } => {}
            }
        }

        state.remove_peer(&remote_id).await;
    });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_handshake(state: &NetworkState) -> ProtocolMessage {
    ProtocolMessage::Handshake {
        node_id: state.node_id.clone(),
        nickname: state.nickname.clone(),
        listen_port: state.listen_port,
    }
}

async fn send_msg(tx: &mpsc::Sender<ProtocolMessage>, msg: &ProtocolMessage) {
    let _ = tx.send(msg.clone()).await;
}

async fn read_line_trimmed(buf: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> io::Result<String> {
    let mut line = String::new();
    buf.read_line(&mut line).await?;
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty line"));
    }
    Ok(trimmed)
}

async fn send_peer_exchange(
    state: &NetworkState,
    write_tx: &mpsc::Sender<ProtocolMessage>,
    remote_id: &str,
) {
    let peers = state.peers.lock().await;
    let list: Vec<PeerAddress> = peers
        .values()
        .filter(|p| p.info.node_id != remote_id)
        .map(|p| p.info.clone())
        .collect();
    if !list.is_empty() {
        let _ = write_tx.send(ProtocolMessage::PeerExchange { peers: list }).await;
    }
}

async fn writer_task(mut writer: tokio::net::tcp::OwnedWriteHalf, mut rx: mpsc::Receiver<ProtocolMessage>) {
    while let Some(msg) = rx.recv().await {
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("[net] serialize error: {}", e);
                continue;
            }
        };
        let line = format!("{}\n", json);
        if writer.write_all(line.as_bytes()).await.is_err() {
            break;
        }
    }
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}