# Decentralized P2P Communication Platform Blueprint

## Goal

Design a censorship-resistant, privacy-focused, Discord-like communication platform with no central servers, end-to-end encryption for all content, cryptographic identities instead of accounts, and native Linux and Windows clients.

## Product Requirements

- No central authority for messaging, discovery, or identity issuance.
- End-to-end encryption for text, voice, attachments, and media.
- Native Linux and Windows support.
- No email, phone number, or personally identifiable information.
- User identity must be portable across devices and independent of any server.
- Support Discord-like spaces, channels, roles, permissions, and voice rooms.
- Operate across NATs and restrictive networks with graceful fallback behavior.
- Provide a roadmap for IP and metadata protection using anonymity networks.

## Non-Goals For The First Release

- Massive public voice rooms with centralized-SFU-level performance.
- Perfect metadata privacy under a global passive adversary.
- Anonymous global user search by default.
- Browser-first support.

## Architecture Overview

The platform should use a local-first, peer-to-peer architecture where every client can act as:

- an identity holder
- a message producer and consumer
- a replication peer
- a relay candidate
- an encrypted storage node for replicated mailbox objects

The system is split into six layers:

1. Identity layer
2. Secure session and key management layer
3. P2P transport and NAT traversal layer
4. Replication and synchronization layer
5. Application state layer
6. Desktop client layer

## Client Platform Strategy

Use a shared Rust core for crypto, storage, networking, and sync. Build native desktop clients for Linux and Windows around that core.

Recommended stack:

- Core: Rust
- UI shell: Tauri or a Rust-native desktop shell
- Local database: SQLite with SQLCipher or an encrypted RocksDB wrapper
- Media codecs: Opus for audio, AV1 or VP9 for video where supported
- Transport: QUIC over UDP

Reasons for Rust:

- single codebase across Linux and Windows
- good ecosystem for QUIC, cryptography, DHTs, and Tor integration
- strong memory safety for network-facing code

## Identity Model

### User Identity

Each user has a long-term root identity keypair.

- Signature key: Ed25519
- Key agreement key: X25519

The canonical user identifier is derived from the public identity key:

`user_id = multihash(identity_public_key)`

This identifier is the account. There is no registration authority.

### Device Model

Each installation creates a device keypair authorized by the root identity key.

- root identity key signs device enrollment
- devices can be revoked by a signed revocation event
- users can export and import an encrypted identity bundle to move between machines

### Portable Account Bundle

The portable account bundle contains:

- root identity private key or recovery material
- authorized device list
- signed prekeys
- space membership state pointers
- local profile metadata

The bundle is encrypted with a user-chosen passphrase and can be transferred by file, QR-assisted workflow, or removable media.

## Key Management

There is no central key server. Clients publish signed identity documents into the distributed network.

### Identity Document

Each identity document contains:

- root public keys
- active device keys
- signed prekeys
- optional one-time prekeys
- supported transport endpoints
- Tor and I2P endpoints if enabled
- expiration timestamp
- signature by the root identity key

### Key Exchange

For direct messaging use asynchronous authenticated key establishment similar to X3DH followed by Double Ratchet.

For groups use Messaging Layer Security.

Recommended crypto patterns:

- Direct messages: X3DH + Double Ratchet
- Group channels: MLS
- File encryption: XChaCha20-Poly1305 with random content keys
- Hashing: BLAKE3 for chunking and content addressing
- KDF: HKDF-SHA256

### Verification

Users can verify contacts by:

- scanning QR fingerprints in person
- comparing short authentication strings
- exchanging signed invites over an out-of-band channel

## P2P Transport And NAT Traversal

### Overlay Network

The network should use a structured overlay and a pub-sub mesh:

- Kademlia-style DHT for discovery and pointer storage
- GossipSub-style dissemination for live channel events
- QUIC for secure transport streams and datagrams

### NAT Traversal Strategy

Direct connectivity is attempted first. Relay-assisted connectivity is the fallback.

Connectivity sequence:

1. Discover peer descriptors from DHT, invite payload, or gossip.
2. Exchange signed reachability candidates.
3. Attempt UDP hole punching using ICE-like checks.
4. Establish QUIC if a direct path succeeds.
5. Fall back to relay peers if symmetric NAT or firewall restrictions prevent direct connectivity.

### Relay Model

No platform-owned relay is required, but relays are still necessary. The system should support volunteer relay peers.

Relay requirements:

- relay advertisements are signed
- clients score relays by latency, uptime, and failure rate
- relays only see encrypted packets and routing metadata
- clients may select multi-hop relays in higher privacy modes

### Protocol Choices

- Transport: QUIC over UDP
- Session handshake: Noise protocol patterns bound to identity keys
- Media datagrams: QUIC datagrams or SRTP-compatible media packets over UDP

## Application Data Model

### Spaces

A Discord-like server is represented as a signed space state.

Each space is made of append-only events:

- space created
- member invited
- member joined
- channel created
- role created
- permission updated
- message posted
- attachment referenced
- call started
- member removed

These events are:

- signed
- causally ordered
- replicated over gossip and checkpoints
- encrypted when appropriate

### Channels

Channel types:

- text
- voice
- media
- announcement
- private subchannel

Each channel can map to its own MLS group to keep rekeying scope bounded.

### Roles And Permissions

Permissions are represented as signed policy events attached to the space state.

Local clients compute effective permissions by replaying the policy log.

Recommended permissions:

- manage space
- manage channels
- manage roles
- invite members
- remove members
- send messages
- upload media
- start voice
- moderate content

## End-To-End Encryption

### Text Messaging

- direct messages use Double Ratchet per peer or per device
- group messages use MLS epochs
- membership changes trigger rekey events

### Voice And Media

Voice requires low latency and should use short-lived session keys derived from a channel call context.

Recommended model:

- small calls use direct peer mesh
- medium calls use partial forwarding by selected peers
- large calls use volunteer forwarders that cannot decrypt media frames

Media keys can be derived from the current MLS epoch exporter secret so rekeying aligns with participant changes.

### Attachments

Attachments are encrypted before distribution.

Process:

1. Generate a random content key.
2. Encrypt file chunks locally.
3. Store encrypted chunks as content-addressed blobs.
4. Share an encrypted key envelope only with authorized recipients.

## Offline Messaging And Synchronization

### Problem

Without central servers, offline delivery must be delegated to replicated peers.

### Distributed Mailboxes

Each user publishes mailbox pointers into the DHT.

Mailbox replicas are encrypted object stores maintained by selected peers and nearby DHT nodes.

Properties:

- replicas cannot decrypt messages
- messages expire after TTL or acknowledgement
- recipient fetches and acknowledges later
- senders can store to multiple replicas for durability

### Sync Strategy

Use hybrid replication:

- gossip for hot, recent channel activity
- DHT for mailbox and identity discovery
- Merkle DAG or hash tree snapshots for history sync
- bloom filters or IBLTs for compact set reconciliation

On reconnect, a client should:

1. fetch mailbox replicas
2. exchange summary structures with known peers
3. request missing encrypted objects
4. merge signed events into local state

### Conflict Resolution

Use CRDTs or append-only signed logs plus deterministic merge rules for:

- channel ordering
- role metadata
- presence hints
- read markers

## Metadata Privacy Roadmap

Content E2EE is necessary but insufficient. The platform should progressively reduce metadata leakage.

### Phase 1

- support direct and privacy-preserving connection modes
- route signaling through Tor when privacy mode is enabled
- suppress public user search by default
- pad sensitive request sizes where practical

### Phase 2

- support Tor hidden services and I2P destinations as peer endpoints
- use anonymous rendezvous for mailbox retrieval
- batch non-urgent traffic to reduce timing correlation
- separate presence updates from message transport

### Phase 3

- add cover traffic for high-risk users
- explore DHT query obfuscation and private lookup schemes
- support per-space enforced anonymity profiles

### Product Modes

Expose three privacy modes:

- Direct: lowest latency, weakest metadata protection
- Balanced: direct where safe, privacy-preserving signaling
- Anonymous: Tor or I2P transport preferred, higher latency tolerated

Voice should default to Balanced because fully anonymous low-latency calls are substantially harder.

## Discovery And Invites

The system should avoid a default global directory.

Primary entry methods:

- signed invite links with bootstrap peers and expiration
- QR code invites
- direct public key exchange
- optional user-published discoverable identity documents

Invite contents:

- space descriptor hash
- bootstrap peer hints
- optional shared secret
- expiration time
- inviter signature

## User Experience Philosophy

The interface should keep the Discord mental model while hiding transport and key-management complexity.

### Layout

- left sidebar for spaces
- channel list pane
- central conversation or call view
- optional member and role pane
- compact connection and encryption status area

### UX Principles

- security is default, not an advanced toggle
- advanced cryptographic state stays out of the main workflow
- delivery failures explain whether the issue is sync, connectivity, or peer availability
- verified identities are visible but not intrusive

### Identity Surface

Each user profile should show:

- display name
- public-key fingerprint
- verification state
- active device count
- current privacy mode

### Space Administration

Admins can:

- create spaces locally
- issue signed invites
- manage channels and roles
- enforce privacy mode recommendations
- rotate membership keys on removal events

## Security Model

### Threats Addressed

- content interception by relays or intermediate peers
- central shutdown or service censorship
- account takeover via server-side credential compromise
- unauthorized history access after membership removal

### Threats Partially Addressed

- traffic analysis
- social graph inference
- Sybil attacks in discovery and relay layers
- abusive users operating many pseudonymous identities

### Required Mitigations

- signed identity documents with expiry
- revocation propagation
- quorum retrieval for DHT reads
- relay reputation and local blocklists
- spam resistance using proof of work or request costs
- local encrypted storage and OS keychain integration

## Native Desktop Design

Linux and Windows clients should package the same Rust core with platform-native secure storage integrations.

Recommended packaging:

- Linux: AppImage and Flatpak
- Windows: MSI or MSIX

Client responsibilities:

- encrypted local database
- background sync worker
- voice and media capture pipeline
- relay and direct connection orchestration
- identity import and export workflows

## Recommended Repository Structure

```text
app-call/
  docs/
    technical-blueprint.md
  core/
    identity/
    crypto/
    transport/
    relay/
    dht/
    sync/
    spaces/
    media/
    storage/
  desktop/
    linux/
    windows/
    shared-ui/
  protocol/
    schemas/
    message-types/
  tests/
    integration/
    network/
    crypto/
```

## Implementation Roadmap

### Milestone 1: Identity And Direct Messaging

- create root and device identity model
- implement signed identity documents
- implement direct messaging with async key exchange and Double Ratchet
- implement encrypted local storage

### Milestone 2: Overlay And Sync

- implement DHT peer and identity discovery
- implement gossip propagation for channel events
- implement mailbox replication and retrieval
- add state checkpointing and history sync

### Milestone 3: Spaces And Permissions

- implement spaces, channels, and roles as signed event logs
- add membership workflows and rekeying
- add attachment encryption and distribution

### Milestone 4: Voice

- implement direct small-group voice
- add NAT traversal diagnostics and relay fallback
- add encrypted forwarding for medium-size rooms

### Milestone 5: Metadata Protection

- integrate Tor for signaling and optional transport
- add I2P support
- add traffic padding and privacy-mode controls

## Open Technical Constraints

- Pure P2P large voice rooms are expensive in bandwidth and fragile under churn.
- Fully serverless offline messaging requires replicated storage peers and cannot guarantee centralized-service reliability.
- Strong anonymity and low-latency voice are in direct tension.
- Relay and DHT layers must be hardened against Sybil and spam attacks before public deployment.

## Decision Summary

The strongest feasible architecture is a local-first P2P system with:

- self-sovereign cryptographic identities
- QUIC-based authenticated peer transport
- X3DH and Double Ratchet for direct chats
- MLS for channels and spaces
- DHT plus gossip for discovery and synchronization
- replicated encrypted mailboxes for offline delivery
- optional Tor and I2P routing for metadata protection

This preserves a familiar Discord-like experience while keeping content encrypted, infrastructure replaceable, and user accounts independent from any server or provider.
