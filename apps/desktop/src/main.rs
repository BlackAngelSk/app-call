use app_core::{AppModel, ChannelKind, NetworkEvent, NetworkState, UserSettings};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(target_os = "linux", target_os = "windows"))]
use eframe::egui;

fn main() {
    let data_dir = resolve_data_dir();
    let startup_log_paths = startup_log_paths(&data_dir);
    reset_startup_logs(&startup_log_paths);
    install_panic_logger(startup_log_paths.clone());
    append_startup_log(
        &startup_log_paths,
        &format!("startup begin; data_dir={}", data_dir.display()),
    );

    let state = app_core::load_or_create_app(&data_dir).unwrap_or_else(|error| {
        append_startup_log(
            &startup_log_paths,
            &format!("failed to load identity: {error}"),
        );
        eprintln!(
            "failed to load persisted identity from {}: {}",
            data_dir.display(),
            error
        );
        std::process::exit(1);
    });
    append_startup_log(&startup_log_paths, "identity loaded");

    let settings = match app_core::load_or_create_user_settings(&data_dir) {
        Ok(settings) => {
            append_startup_log(&startup_log_paths, "settings loaded");
            settings
        }
        Err(error) => {
            append_startup_log(
                &startup_log_paths,
                &format!("failed to load settings, using defaults: {error}"),
            );
            UserSettings::default()
        }
    };

    // Force console mode with APP_CALL_CONSOLE=1 or if no display is available.
    let force_console = env::var("APP_CALL_CONSOLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let has_display = {
        #[cfg(target_os = "linux")]
        {
            env::var("DISPLAY").is_ok()
                || env::var("WAYLAND_DISPLAY").is_ok()
                || env::var("APP_CALL_FORCE_GUI").is_ok()
        }
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
    };

    if force_console || !has_display {
        append_startup_log(
            &startup_log_paths,
            &format!(
                "console mode: force_console={}, has_display={}",
                force_console, has_display
            ),
        );
        run_networked_console(&state, &settings, &startup_log_paths);
        return;
    }

    // Try GUI first; fall back to networked console if unavailable.
    if let Err(error) = run_app(state.clone(), data_dir.clone(), settings.clone()) {
        append_startup_log(&startup_log_paths, &format!("failed to start UI: {error}"));
        write_startup_error(&data_dir, &error.to_string());

        if is_graphics_backend_unavailable(&error.to_string()) {
            append_startup_log(&startup_log_paths, "starting networked console mode");
            run_networked_console(&state, &settings, &startup_log_paths);
            return;
        }

        eprintln!("failed to start UI: {error}");
        std::process::exit(1);
    }

    append_startup_log(&startup_log_paths, "app exited cleanly");
}

// ── Networked console mode ──────────────────────────────────────────────────

fn run_networked_console(app: &AppModel, _settings: &UserSettings, log_paths: &[PathBuf]) {
    let listen_port: u16 = env::var("APP_CALL_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9000);

    let nickname = app.local_identity.display_name.clone();
    let node_id = app.local_identity.identity_id().short().to_string();

    let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Failed to create tokio runtime: {}", e);
        std::process::exit(1);
    });

    rt.block_on(async move {
        let (net_state, mut event_rx) = NetworkState::new(node_id, nickname, listen_port);
        let net = Arc::new(net_state);

        // Start TCP listener.
        if let Err(e) = net.start_listener().await {
            eprintln!("Failed to start listener: {}", e);
            std::process::exit(1);
        }

        // Print welcome banner.
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║               app-call — P2P Console Mode               ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        println!("  Nickname : {}", app.local_identity.display_name);
        println!("  Node ID  : {}", app.local_identity.identity_id().short());
        println!("  Listen   : 0.0.0.0:{}", listen_port);
        println!();
        println!("  Commands:");
        println!("    connect <ip:port>    Connect to a peer");
        println!("    msg <text>           Send a message to all peers");
        println!("    peers                List connected peers");
        println!("    myid                 Show your identity info");
        println!("    port                 Show listening port");
        println!("    help                 Show this help");
        println!("    quit / exit          Exit");
        println!();
        println!("  To connect from another machine:");
        println!("    connect <this-machine-ip>:{}", listen_port);
        println!();
        println!("  Type a command to get started.");
        println!("────────────────────────────────────────────────────────────");

        // Spawn a task to read stdin and forward lines to the main loop.
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<String>(32);
        tokio::task::spawn_blocking(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                if stdin.read_line(&mut line).is_err() {
                    break;
                }
                let trimmed = line.trim().to_string();
                if stdin_tx.blocking_send(trimmed).is_err() {
                    break;
                }
            }
        });

        // Main event loop.
        loop {
            tokio::select! {
                // User input.
                Some(line) = stdin_rx.recv() => {
                    if line.is_empty() {
                        continue;
                    }

                    if line == "quit" || line == "exit" {
                        println!("Goodbye.");
                        break;
                    }

                    if line == "help" {
                        println!();
                        println!("  Commands:");
                        println!("    connect <ip:port>    Connect to a peer");
                        println!("    msg <text>           Send a message to all peers");
                        println!("    peers                List connected peers");
                        println!("    myid                 Show your identity info");
                        println!("    port                 Show listening port");
                        println!("    help                 Show this help");
                        println!("    quit / exit          Exit");
                        println!();
                        continue;
                    }

                    if line == "myid" {
                        println!("  Nickname : {}", app.local_identity.display_name);
                        println!("  Node ID  : {}", app.local_identity.identity_id());
                        println!("  Listen   : 0.0.0.0:{}", listen_port);
                        continue;
                    }

                    if line == "port" {
                        println!("  Listening on port {}", listen_port);
                        continue;
                    }

                    if line == "peers" {
                        let peers = net.list_peers().await;
                        if peers.is_empty() {
                            println!("  No peers connected.");
                        } else {
                            println!("  Connected peers ({}):", peers.len());
                            for p in &peers {
                                println!("    {} ({})", p.nickname, p.addr);
                                println!("      ID: {}", p.node_id);
                            }
                        }
                        continue;
                    }

                    if let Some(rest) = line.strip_prefix("connect ") {
                        let addr = rest.trim();
                        if addr.is_empty() {
                            println!("  Usage: connect <ip:port>");
                            continue;
                        }
                        println!("  Connecting to {}...", addr);
                        match net.connect_peer(addr).await {
                            Ok(()) => {
                                println!("  Connection initiated.");
                            }
                            Err(e) => {
                                println!("  Failed to connect: {}", e);
                            }
                        }
                        continue;
                    }

                    if let Some(rest) = line.strip_prefix("msg ") {
                        let body = rest.trim();
                        if body.is_empty() {
                            println!("  Usage: msg <text>");
                            continue;
                        }
                        net.send_chat(body).await;
                        println!("  [you] {}", body);
                        continue;
                    }

                    // If the line doesn't match a command, treat it as a message
                    // (convenience: just type text and it sends).
                    if !line.starts_with('/') {
                        net.send_chat(&line).await;
                        println!("  [you] {}", line);
                        continue;
                    }

                    println!("  Unknown command: {}", line);
                    println!("  Type 'help' for available commands.");
                }

                // Network events.
                Ok(event) = event_rx.recv() => {
                    match event {
                        NetworkEvent::ChatReceived(chat) => {
                            println!("  [{}] {}", chat.from_name, chat.body);
                        }
                        NetworkEvent::PeerConnected { nickname, addr, .. } => {
                            println!("  >> {} connected ({})", nickname, addr);
                        }
                        NetworkEvent::PeerDisconnected { nickname, .. } => {
                            println!("  << {} disconnected", nickname);
                        }
                        NetworkEvent::Info(msg) => {
                            println!("  [info] {}", msg);
                        }
                        NetworkEvent::Error(msg) => {
                            println!("  [error] {}", msg);
                        }
                    }
                }

                else => {
                    // Both channels closed.
                    break;
                }
            }
        }
    });

    append_startup_log(log_paths, "networked console mode exited");
}

// ── GUI mode (preserved for systems with GPU) ───────────────────────────────

fn run_app(state: AppModel, data_dir: PathBuf, settings: UserSettings) -> Result<(), String> {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1080.0, 680.0]),
            hardware_acceleration: eframe::HardwareAcceleration::Preferred,
            ..Default::default()
        };

        eframe::run_native(
            "app-call",
            options,
            Box::new(move |_cc| Ok(Box::new(GuiApp::new(state, data_dir, settings)))),
        )
        .map_err(|e| e.to_string())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err("GUI not supported on this platform".to_string())
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
struct GuiApp {
    app: AppModel,
    data_dir: PathBuf,
    selected_space: usize,
    selected_channel: Option<usize>,
    composer_text: String,
    pending_display_name: String,
    profile_status: Option<String>,
    settings: UserSettings,
    settings_status: Option<String>,
    timelines: HashMap<String, Vec<GuiMessage>>,
    users_by_space: HashMap<usize, Vec<String>>,
    voice_participants: HashMap<String, Vec<String>>,
    active_voice: Option<GuiChannelSelection>,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Clone, Debug)]
struct GuiMessage {
    author: String,
    body: String,
    timestamp: String,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuiChannelSelection {
    space_index: usize,
    channel_index: usize,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl GuiApp {
    fn new(app: AppModel, data_dir: PathBuf, settings: UserSettings) -> Self {
        let mut timelines = HashMap::new();
        let mut users_by_space = HashMap::new();
        let local_username = app.local_identity.display_name.clone();

        for (space_index, space) in app.spaces.iter().enumerate() {
            for (channel_index, channel) in space.channels.iter().enumerate() {
                if matches!(channel.kind, ChannelKind::Text | ChannelKind::Announcement) {
                    let key = gui_channel_key(space_index, channel_index);
                    timelines.insert(
                        key,
                        vec![GuiMessage {
                            author: "system".to_string(),
                            body: format!("Welcome to #{}", channel.name),
                            timestamp: now_timestamp(),
                        }],
                    );
                }
            }
            users_by_space.insert(space_index, vec![local_username.clone()]);
        }

        let selected_channel = app
            .spaces
            .first()
            .and_then(|space| (!space.channels.is_empty()).then_some(0));

        let pending_display_name = app.local_identity.display_name.clone();

        Self {
            app,
            data_dir,
            selected_space: 0,
            selected_channel,
            composer_text: String::new(),
            pending_display_name,
            profile_status: None,
            settings,
            settings_status: None,
            timelines,
            users_by_space,
            voice_participants: HashMap::new(),
            active_voice: None,
        }
    }

    fn select_space(&mut self, new_index: usize) {
        self.selected_space = new_index;
        self.selected_channel = self
            .app
            .spaces
            .get(new_index)
            .and_then(|space| (!space.channels.is_empty()).then_some(0));
    }

    fn is_active_voice_channel(&self, channel_index: usize) -> bool {
        matches!(
            self.active_voice,
            Some(GuiChannelSelection {
                space_index,
                channel_index: active_channel_index,
            }) if space_index == self.selected_space && active_channel_index == channel_index
        )
    }

    fn send_message(&mut self, channel_index: usize) {
        let trimmed = self.composer_text.trim();
        if trimmed.is_empty() {
            return;
        }

        let key = gui_channel_key(self.selected_space, channel_index);
        let author = self.app.local_identity.display_name.clone();
        self.timelines.entry(key).or_default().push(GuiMessage {
            author,
            body: trimmed.to_string(),
            timestamp: now_timestamp(),
        });
        self.composer_text.clear();
    }

    fn toggle_voice_channel(&mut self, channel_index: usize) {
        let selection = GuiChannelSelection {
            space_index: self.selected_space,
            channel_index,
        };

        if self.is_active_voice_channel(channel_index) {
            self.leave_voice(selection);
            self.active_voice = None;
            self.profile_status = Some("left voice chat".to_string());
        } else {
            if let Some(active) = self.active_voice.take() {
                self.leave_voice(active);
            }
            self.join_voice(selection);
            self.active_voice = Some(selection);
            self.profile_status = Some("joined voice chat".to_string());
        }
    }

    fn join_voice(&mut self, selection: GuiChannelSelection) {
        let username = self.app.local_identity.display_name.clone();
        let channel_id = gui_channel_key(selection.space_index, selection.channel_index);
        let participants = self.voice_participants.entry(channel_id).or_default();
        if !participants.iter().any(|entry| entry == &username) {
            participants.push(username.clone());
        }
        let users = self.users_by_space.entry(selection.space_index).or_default();
        if !users.iter().any(|entry| entry == &username) {
            users.push(username);
        }
    }

    fn leave_voice(&mut self, selection: GuiChannelSelection) {
        let username = self.app.local_identity.display_name.clone();
        let channel_id = gui_channel_key(selection.space_index, selection.channel_index);
        if let Some(participants) = self.voice_participants.get_mut(&channel_id) {
            participants.retain(|entry| entry != &username);
            if participants.is_empty() {
                self.voice_participants.remove(&channel_id);
            }
        }
    }

    fn save_display_name(&mut self) {
        let trimmed = self.pending_display_name.trim();
        if trimmed.is_empty() {
            self.profile_status = Some("username cannot be empty".to_string());
            return;
        }

        let previous_name = self.app.local_identity.display_name.clone();
        match app_core::update_display_name(&self.data_dir, trimmed) {
            Ok(()) => {
                self.app.local_identity.display_name = trimmed.to_string();
                for usernames in self.users_by_space.values_mut() {
                    for username in usernames.iter_mut() {
                        if username == &previous_name {
                            *username = self.app.local_identity.display_name.clone();
                        }
                    }
                }
                for participants in self.voice_participants.values_mut() {
                    for username in participants.iter_mut() {
                        if username == &previous_name {
                            *username = self.app.local_identity.display_name.clone();
                        }
                    }
                }
                self.profile_status = Some("username updated".to_string());
            }
            Err(error) => {
                self.profile_status = Some(format!("failed to update username: {error}"));
            }
        }
    }

    fn save_settings(&mut self) {
        match app_core::save_user_settings(&self.data_dir, &self.settings) {
            Ok(()) => {
                self.settings_status = Some("settings saved".to_string());
            }
            Err(error) => {
                self.settings_status = Some(format!("failed to save settings: {error}"));
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl eframe::App for GuiApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if self.settings.dark_theme {
            context.set_visuals(egui::Visuals::dark());
        } else {
            context.set_visuals(egui::Visuals::light());
        }

        egui::TopBottomPanel::top("top_bar").show(context, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("app-call");
                ui.separator();
                ui.label(format!("Identity: {}", self.app.local_identity.display_name));
                ui.separator();
                ui.label(format!("Privacy: {}", self.app.local_identity.privacy_mode));
                ui.separator();
                ui.monospace(format!("ID: {}", self.app.local_identity.identity_id().short()));
                ui.separator();
                ui.label(format!("Devices: {}", self.app.local_identity.devices.len()));
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Username:");
                ui.add_sized(
                    [220.0, 26.0],
                    egui::TextEdit::singleline(&mut self.pending_display_name),
                );

                let save_clicked = ui.button("Save").clicked();
                let enter_pressed =
                    ui.input(|input_state| input_state.key_pressed(egui::Key::Enter));
                if save_clicked || enter_pressed {
                    self.save_display_name();
                }

                if let Some(status) = &self.profile_status {
                    ui.separator();
                    ui.label(status);
                }
            });
        });

        egui::SidePanel::left("spaces")
            .resizable(true)
            .default_width(220.0)
            .show(context, |ui| {
                ui.heading("Spaces");
                ui.add_space(8.0);

                let space_names: Vec<String> =
                    self.app.spaces.iter().map(|space| space.name.clone()).collect();

                for (index, name) in space_names.iter().enumerate() {
                    let selected = self.selected_space == index;
                    if ui.selectable_label(selected, name).clicked() {
                        self.select_space(index);
                    }
                }
            });

        egui::SidePanel::right("members")
            .resizable(true)
            .default_width(220.0)
            .show(context, |ui| {
                ui.heading("Usernames");
                ui.add_space(8.0);

                let users = self
                    .users_by_space
                    .get(&self.selected_space)
                    .cloned()
                    .unwrap_or_default();

                for username in users {
                    ui.label(format!("@{username}"));
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                ui.heading("Settings");
                ui.checkbox(&mut self.settings.dark_theme, "Dark theme");
                ui.checkbox(&mut self.settings.enter_to_send, "Enter sends message");
                ui.checkbox(&mut self.settings.show_timestamps, "Show timestamps");
                ui.checkbox(&mut self.settings.auto_join_voice, "Auto join voice on open");

                if ui.button("Save Settings").clicked() {
                    self.save_settings();
                }

                if let Some(status) = &self.settings_status {
                    ui.label(status);
                }
            });

        egui::CentralPanel::default().show(context, |ui| {
            let Some((space_name, member_count, channels)) = self
                .app
                .spaces
                .get(self.selected_space)
                .map(|space| {
                    (
                        space.name.clone(),
                        space.member_count,
                        space.channels.clone(),
                    )
                })
            else {
                ui.heading("No spaces yet");
                return;
            };

            ui.heading(&space_name);
            ui.label(format!("{} members", member_count));
            ui.label(format!("{} channels", channels.len()));
            ui.add_space(10.0);

            egui::Grid::new("channels_grid")
                .num_columns(5)
                .striped(true)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.strong("Channel");
                    ui.strong("Type");
                    ui.strong("Security");
                    ui.strong("Action");
                    ui.strong("State");
                    ui.end_row();

                    for (channel_index, channel) in channels.iter().enumerate() {
                        let is_selected = self.selected_channel == Some(channel_index);
                        let mut selected_clicked = false;
                        if ui
                            .selectable_label(is_selected, format!("# {}", channel.name))
                            .clicked()
                        {
                            self.selected_channel = Some(channel_index);
                            selected_clicked = true;
                        }

                        if selected_clicked
                            && self.settings.auto_join_voice
                            && matches!(channel.kind, ChannelKind::Voice)
                            && !self.is_active_voice_channel(channel_index)
                        {
                            self.toggle_voice_channel(channel_index);
                        }

                        ui.label(gui_channel_kind_label(channel.kind));
                        ui.colored_label(
                            if channel.encrypted {
                                egui::Color32::from_rgb(72, 166, 94)
                            } else {
                                egui::Color32::from_rgb(200, 96, 96)
                            },
                            if channel.encrypted { "E2EE" } else { "Open" },
                        );

                        match channel.kind {
                            ChannelKind::Voice => {
                                let joined = self.is_active_voice_channel(channel_index);
                                let label = if joined { "Leave" } else { "Join" };
                                if ui.button(label).clicked() {
                                    self.toggle_voice_channel(channel_index);
                                }
                            }
                            _ => {
                                if ui.button("Open").clicked() {
                                    self.selected_channel = Some(channel_index);
                                }
                            }
                        }

                        ui.label(match channel.kind {
                            ChannelKind::Voice if self.is_active_voice_channel(channel_index) => {
                                "In call"
                            }
                            _ => "Ready",
                        });
                        ui.end_row();
                    }
                });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            let Some(channel_index) = self.selected_channel else {
                ui.label("Select a channel to start chatting.");
                return;
            };

            let Some(channel) = channels.get(channel_index) else {
                ui.label("Select a channel to start chatting.");
                return;
            };

            match channel.kind {
                ChannelKind::Voice => {
                    ui.heading(format!("Voice: {}", channel.name));
                    let joined = self.is_active_voice_channel(channel_index);
                    ui.label(if joined {
                        "Connected to voice chat"
                    } else {
                        "Not connected"
                    });

                    let button_label = if joined {
                        "Leave voice chat"
                    } else {
                        "Join voice chat"
                    };
                    if ui.button(button_label).clicked() {
                        self.toggle_voice_channel(channel_index);
                    }

                    ui.add_space(10.0);
                    let key = gui_channel_key(self.selected_space, channel_index);
                    let participants = self.voice_participants.get(&key).cloned().unwrap_or_default();
                    ui.label(format!("Participants: {}", participants.len()));
                    if participants.is_empty() {
                        ui.label("No users in voice chat");
                    } else {
                        for username in participants {
                            ui.label(format!("@{username}"));
                        }
                    }
                }
                _ => {
                    ui.heading(format!("Chat: {}", channel.name));
                    let key = gui_channel_key(self.selected_space, channel_index);
                    let timeline = self.timelines.entry(key).or_default();

                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for message in timeline.iter() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(format!("{}:", message.author));
                                    ui.label(&message.body);
                                    if self.settings.show_timestamps {
                                        ui.label(format!("({})", message.timestamp));
                                    }
                                });
                                ui.add_space(4.0);
                            }
                        });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let input = ui.add_sized(
                            [ui.available_width() - 90.0, 28.0],
                            egui::TextEdit::singleline(&mut self.composer_text)
                                .hint_text("Type a message"),
                        );

                        let enter_pressed = self.settings.enter_to_send
                            && input.lost_focus()
                            && ui.input(|input_state| input_state.key_pressed(egui::Key::Enter));

                        let send_clicked = ui
                            .add_enabled(
                                !self.composer_text.trim().is_empty(),
                                egui::Button::new("Send"),
                            )
                            .clicked();

                        if send_clicked || enter_pressed {
                            self.send_message(channel_index);
                        }
                    });
                }
            }
        });
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn gui_channel_key(space_index: usize, channel_index: usize) -> String {
    format!("{space_index}:{channel_index}")
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn gui_channel_kind_label(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::Text => "# text",
        ChannelKind::Voice => "voice",
        ChannelKind::Media => "media",
        ChannelKind::Announcement => "announce",
    }
}

// ── Shared helpers ──────────────────────────────────────────────────────────

fn write_startup_error(data_dir: &PathBuf, error: &str) {
    let _ = fs::create_dir_all(data_dir);
    let _ = fs::write(data_dir.join("startup-error.log"), error);
}

fn is_graphics_backend_unavailable(error: &str) -> bool {
    let lower = error.to_lowercase();
    // Catch any GPU / graphics / rendering failure so we can fall back to
    // console mode.  This is intentionally broad: on VMs and headless
    // machines the exact error message varies by OS, driver, and backend
    // (wgpu / glow / D3D12 / DXGI / Vulkan / EGL / OpenGL / …).
    lower.contains("no suitable adapter found")
        || lower.contains("requires opengl 2.0+")
        || lower.contains("failed to create wgpu adapter")
        || lower.contains("no backend")
        || lower.contains("egl")
        || lower.contains("vulkan")
        || lower.contains("d3d12")
        || lower.contains("dxgi")
        || lower.contains("direct3d")
        || lower.contains("directx")
        || lower.contains("opengl")
        || lower.contains("gles")
        || lower.contains("angle")
        || lower.contains("adapter")
        || lower.contains("surface")
        || lower.contains("swapchain")
        || lower.contains("device lost")
        || lower.contains("device removed")
        || lower.contains("gpu")
        || lower.contains("wgpu")
        || lower.contains("glow")
        || lower.contains("render")
        || lower.contains("graphics")
        || lower.contains("window")
        || lower.contains("display")
        || lower.contains("monitor")
        || lower.contains("headless")
}

fn startup_log_paths(data_dir: &std::path::Path) -> Vec<PathBuf> {
    vec![
        PathBuf::from("app-call-startup.log"),
        data_dir.join("startup-error.log"),
    ]
}

fn reset_startup_logs(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn append_startup_log(paths: &[PathBuf], message: &str) {
    let line = format!("[{}] {}\n", now_timestamp(), message);

    for path in paths {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

fn install_panic_logger(paths: Vec<PathBuf>) {
    std::panic::set_hook(Box::new(move |panic_info| {
        append_startup_log(&paths, &format!("panic: {panic_info}"));
    }));
}

fn resolve_data_dir() -> PathBuf {
    if let Ok(path) = env::var("APP_CALL_DATA_DIR") {
        return PathBuf::from(path);
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = env::var("APPDATA") {
            return PathBuf::from(app_data).join("app-call");
        }

        if let Ok(user_profile) = env::var("USERPROFILE") {
            return PathBuf::from(user_profile).join("AppData/Roaming/app-call");
        }
    }

    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join(".local/share/app-call");
    }

    PathBuf::from(".app-call-data")
}

fn now_timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}", duration.as_secs()),
        Err(_) => "0".to_string(),
    }
}