mod net_monitor;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Modifier},
    symbols,
    widgets::{Block, Borders, List, ListItem, Paragraph, BorderType, canvas::{Canvas, Line}, ListState, Clear},
    Terminal, Frame,
};
use crossterm::{
    event::{self, Event, KeyCode, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io::{self, Write}, time::{Duration, Instant}, process::{Command, Stdio}, collections::HashMap};

#[derive(PartialEq, Clone, Copy)]
enum SelectionMode {
    VPN,
    WiFi,
    PasswordInput,
}

struct InterfaceData {
    history: Vec<(f64, f64)>,
    current_speed: f64,
    color: Color,
}

struct App {
    vpn_names: Vec<String>,
    wifi_ssids: Vec<String>,
    active_vpns: Vec<String>,
    previous_active_vpns: Vec<String>,
    current_ssid: String,
    previous_ssid: String,
    selection_mode: SelectionMode,
    previous_mode: SelectionMode,
    password_input: String,
    list_state: ListState,
    interfaces: HashMap<String, InterfaceData>,
    last_stats: HashMap<String, net_monitor::NetStats>,
    counter: f64,
    graph_index: usize, 
}

impl App {
    fn new() -> Self {
        let mut app = App {
            vpn_names: Self::get_nm_vpn_connections(),
            wifi_ssids: Self::scan_wifi_ssids(),
            active_vpns: Vec::new(),
            previous_active_vpns: Vec::new(),
            current_ssid: String::new(),
            previous_ssid: String::new(),
            selection_mode: SelectionMode::VPN,
            previous_mode: SelectionMode::VPN,
            password_input: String::new(),
            list_state: ListState::default(),
            interfaces: HashMap::new(),
            last_stats: net_monitor::get_net_data(),
            counter: 0.0,
            graph_index: 0,
        };
        app.list_state.select(Some(0));
        app.update_active_states();
        app
    }

    fn send_notification(summary: &str, body: &str, critical: bool) {
        let urgency = if critical { "critical" } else { "normal" };
        let icon = if critical { "network-error" } else { "network-transmit-receive" };
        let _ = Command::new("notify-send").args(["-u", urgency, "-i", icon, summary, body]).spawn();
    }

    fn get_nm_vpn_connections() -> Vec<String> {
        let output = Command::new("nmcli").args(["-t", "-f", "NAME,TYPE", "connection", "show"]).output();
        if let Ok(out) = output {
            let s = String::from_utf8_lossy(&out.stdout);
            let mut names: Vec<String> = s.lines()
                .filter(|line| line.contains(":vpn") || line.contains(":wireguard"))
                .map(|line| line.split(':').next().unwrap_or("").to_string()).collect();
            names.sort(); names
        } else { Vec::new() }
    }

    fn scan_wifi_ssids() -> Vec<String> {
        let output = Command::new("nmcli").args(["-t", "-f", "SSID", "dev", "wifi", "list"]).output();
        if let Ok(out) = output {
            let s = String::from_utf8_lossy(&out.stdout);
            let mut ssids: Vec<String> = s.lines().filter(|l| !l.is_empty() && *l != "--").map(|s| s.to_string()).collect();
            ssids.sort(); ssids.dedup(); ssids
        } else { Vec::new() }
    }

    fn update_active_states(&mut self) {
        self.previous_active_vpns = self.active_vpns.clone();
        if let Ok(out) = Command::new("nmcli").args(["-t", "-f", "NAME,STATE", "con", "show", "--active"]).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            self.active_vpns = s.lines().map(|l| l.split(':').next().unwrap_or("").to_string()).filter(|n| !n.is_empty()).collect();
        }
        if let Ok(out) = Command::new("nmcli").args(["-t", "-f", "ACTIVE,SSID", "dev", "wifi"]).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            self.current_ssid = s.lines().find(|l| l.starts_with("yes")).map(|l| l.split(':').nth(1).unwrap_or("").to_string()).unwrap_or_default();
        }
        for vpn in &self.previous_active_vpns {
            if !self.active_vpns.contains(vpn) { Self::send_notification("VPN Déconnecté", &format!("Tunnel '{}' fermé.", vpn), true); }
        }
        for vpn in &self.active_vpns {
            if !self.previous_active_vpns.contains(vpn) { Self::send_notification("VPN Connecté", &format!("Tunnel '{}' actif.", vpn), false); }
        }
    }

    fn update_metrics(&mut self) {
        self.update_active_states();
        let current_stats = net_monitor::get_net_data();
        self.counter += 1.0;
        for (name, stats) in current_stats.iter() {
            if name == "lo" || name.contains("docker") || name.contains("br-") { continue; }
            if let Some(old_stats) = self.last_stats.get(name) {
                let speed = ((stats.rx.saturating_sub(old_stats.rx) as f64) * 8.0) / (1024.0 * 1024.0);
                let entry = self.interfaces.entry(name.clone()).or_insert(InterfaceData {
                    history: Vec::new(),
                    current_speed: 0.0,
                    color: if name.starts_with('w') { Color::Yellow } else if name.starts_with('e') { Color::Green } else { Color::Cyan },
                });
                entry.current_speed = speed;
                entry.history.push((self.counter, speed));
                if entry.history.len() > 300 { entry.history.remove(0); }
            }
        }
        self.last_stats = current_stats;
        self.interfaces.retain(|name, _| self.last_stats.contains_key(name));
    }

    fn get_active_ips(&self) -> Vec<(String, String)> {
        let mut ips = Vec::new();
        if let Ok(out) = Command::new("ip").args(["-4", "-o", "addr", "show"]).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let name = parts[1].to_string();
                    let ip = parts[3].split('/').next().unwrap_or("").to_string();
                    if name != "lo" { ips.push((name, ip)); }
                }
            }
        }
        ips
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        let timeout = tick_rate.checked_sub(last_tick.elapsed()).unwrap_or(Duration::ZERO);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if app.selection_mode == SelectionMode::PasswordInput {
                    match key.code {
                        KeyCode::Enter => {
                            let secret = app.password_input.clone();
                            let idx = app.list_state.selected().unwrap_or(0);
                            let target = if app.previous_mode == SelectionMode::VPN { app.vpn_names.get(idx).cloned() } else { app.wifi_ssids.get(idx).cloned() };
                            if let Some(name) = target {
                                let mut child = if app.previous_mode == SelectionMode::VPN { 
                                    Command::new("nmcli").args(["con", "up", "id", &name, "--ask"])
                                        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()? 
                                } else {
                                    Command::new("nmcli").args(["dev", "wifi", "connect", &name, "--ask"])
                                        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()? 
                                };
                                if let Some(mut stdin) = child.stdin.take() { let _ = writeln!(stdin, "{}", secret); }
                            }
                            app.selection_mode = app.previous_mode;
                        }
                        KeyCode::Esc => app.selection_mode = app.previous_mode,
                        KeyCode::Backspace => { app.password_input.pop(); }
                        KeyCode::Char(c) => { app.password_input.push(c); }
                        _ => {}
                    }
                } else {
                    let list_len = if app.selection_mode == SelectionMode::VPN { app.vpn_names.len() } else { app.wifi_ssids.len() };
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Tab => { app.selection_mode = if app.selection_mode == SelectionMode::VPN { SelectionMode::WiFi } else { SelectionMode::VPN }; app.list_state.select(Some(0)); }
                        KeyCode::Down | KeyCode::Char('j') => if list_len > 0 {
                            let i = match app.list_state.selected() { Some(i) => if i >= list_len - 1 { 0 } else { i + 1 }, None => 0 };
                            app.list_state.select(Some(i));
                        }
                        KeyCode::Up | KeyCode::Char('k') => if list_len > 0 {
                            let i = match app.list_state.selected() { Some(i) => if i == 0 { list_len - 1 } else { i - 1 }, None => 0 };
                            app.list_state.select(Some(i));
                        }
                        KeyCode::Enter => if list_len > 0 { app.previous_mode = app.selection_mode; app.selection_mode = SelectionMode::PasswordInput; app.password_input.clear(); }
                        KeyCode::Char('x') => if app.selection_mode == SelectionMode::VPN { 
                            if let Some(idx) = app.list_state.selected() {
                                if let Some(name) = app.vpn_names.get(idx) { 
                                    let _ = Command::new("nmcli").args(["con", "down", "id", name])
                                        .stdout(Stdio::null()).stderr(Stdio::null()).spawn(); 
                                }
                            }
                        }
                        KeyCode::Char('r') => { app.vpn_names = App::get_nm_vpn_connections(); app.wifi_ssids = App::scan_wifi_ssids(); }
                        KeyCode::Char('g') => { app.graph_index += 1; }
                        KeyCode::Char('a') => { let _ = Command::new("nm-connection-editor").stdout(Stdio::null()).stderr(Stdio::null()).spawn(); }
                        _ => {}
                    }
                }
            }
        }
        if last_tick.elapsed() >= tick_rate { app.update_metrics(); last_tick = Instant::now(); }
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Percentage(60), 
        Constraint::Percentage(30), 
        Constraint::Length(3)
    ]).split(f.size());

    let top_chunks = Layout::default().direction(Direction::Horizontal).constraints([
        Constraint::Percentage(40), 
        Constraint::Percentage(60)
    ]).split(main_chunks[0]);

    let (title, items) = match app.selection_mode {
        SelectionMode::WiFi => (" [ WIFI SCAN ] ", app.wifi_ssids.iter().map(|s| {
            let active = s == &app.current_ssid;
            ListItem::new(format!(" {} {}", if active { "📶" } else { "  " }, s)).style(if active { Style::default().fg(Color::Yellow) } else { Style::default() })
        }).collect::<Vec<ListItem>>()),
        _ => (" [ VPN LIST ] ", app.vpn_names.iter().map(|s| {
            let active = app.active_vpns.contains(s);
            ListItem::new(format!(" {} {}", if active { "●" } else { "○" }, s)).style(if active { Style::default().fg(Color::Cyan) } else { Style::default() })
        }).collect::<Vec<ListItem>>()),
    };

    let list_widget = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL).border_type(BorderType::Thick).border_style(Style::default().fg(if app.selection_mode == SelectionMode::WiFi { Color::Yellow } else { Color::Cyan })))
        .highlight_style(Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list_widget, top_chunks[0], &mut app.list_state);

    let active_ips = app.get_active_ips();
    let ifs: Vec<ListItem> = active_ips.iter().map(|(n, ip)| {
        let color = if n.starts_with("tun") || n.starts_with("wg") { Color::Cyan } else { Color::Green };
        ListItem::new(format!(" • {:<15}: {}", n, ip)).style(Style::default().fg(color))
    }).collect();
    f.render_widget(List::new(ifs).block(Block::default().title(" [ ACTIVE INTERFACES ] ").borders(Borders::ALL)), top_chunks[1]);

    let mut ifaces_with_ip: Vec<_> = app.interfaces.iter()
        .filter(|(name, _)| active_ips.iter().any(|(ip_name, _)| ip_name == *name))
        .collect();
    let mut physical_active: Vec<_> = ifaces_with_ip.iter().filter(|(n, _)| n.starts_with('e') || n.starts_with('w')).collect();
    let mut tunnel_active: Vec<_> = ifaces_with_ip.iter().filter(|(n, _)| n.starts_with("tun") || n.starts_with("wg") || n.starts_with("ppp")).collect();

    physical_active.sort_by_key(|(n, _)| (*n).clone());
    tunnel_active.sort_by_key(|(n, _)| (*n).clone());

    if app.graph_index % 2 != 0 && !tunnel_active.is_empty() {
        let (name, data) = tunnel_active[(app.graph_index / 2) % tunnel_active.len()];
        render_braille_graph(f, main_chunks[1], name, data.current_speed, &data.history, Color::Cyan, app.counter);
    } else if let Some((name, data)) = physical_active.first() {
        render_braille_graph(f, main_chunks[1], name, data.current_speed, &data.history, data.color, app.counter);
    } else {
        f.render_widget(Paragraph::new("Attente d'une IP active...").alignment(ratatui::layout::Alignment::Center).block(Block::default().borders(Borders::ALL)), main_chunks[1]);
    }

    f.render_widget(Paragraph::new(" [TAB] Mode | [G] Graph | [A] Add VPN | [ENTER] Connect | [X] Disc | [Q] Quit ").block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)).style(Style::default().fg(Color::Gray)), main_chunks[2]);

    if app.selection_mode == SelectionMode::PasswordInput {
        let area = centered_rect(50, 20, f.size());
        f.render_widget(Clear, area);
        f.render_widget(Paragraph::new("*".repeat(app.password_input.len())).block(Block::default().title(" Password Required ").borders(Borders::ALL).border_style(Style::default().fg(Color::Magenta)).border_type(BorderType::Double)).alignment(ratatui::layout::Alignment::Center), area);
    }
}

fn centered_rect(px: u16, py: u16, r: Rect) -> Rect {
    let layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Percentage((100-py)/2), Constraint::Percentage(py), Constraint::Percentage((100-py)/2)]).split(r);
    Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage((100-px)/2), Constraint::Percentage(px), Constraint::Percentage((100-px)/2)]).split(layout[1])[1]
}

fn render_braille_graph(f: &mut Frame, area: Rect, interface: &str, speed: f64, data: &[(f64, f64)], color: Color, last_x: f64) {
    let max_val = data.iter().map(|&(_, y)| y).fold(1.0, f64::max).max(1.0);
    let canvas = Canvas::default().block(Block::default().title(format!(" {} - {:.2} Mb/s ", interface, speed)).borders(Borders::ALL).border_type(BorderType::Rounded))
        .marker(symbols::Marker::Braille).x_bounds([last_x - 300.0, last_x]).y_bounds([0.0, max_val])
        .paint(|ctx| {
            ctx.print(last_x - 295.0, max_val * 0.7, format!("{:.1} Mb/s max", max_val));
            for i in 0..data.len().saturating_sub(1) {
                ctx.draw(&Line { x1: data[i].0, y1: data[i].1, x2: data[i+1].0, y2: data[i+1].1, color });
            }
        });
    f.render_widget(canvas, area);
}
