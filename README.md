🚀 DashNet - NetMonitor TUI

NetMonitor TUI is a lightweight Terminal User Interface (TUI) network and VPN manager written in Rust. It provides real-time bandwidth visualization, Wi-Fi scanning, and VPN management via NetworkManager, all within a sleek and responsive interface.
✨ Features

    📊 Real-time Graphs: High-precision bandwidth tracking (Mb/s) using Braille-based rendering.

    🔒 VPN Management: List, connect, and disconnect VPN profiles (OpenVPN, WireGuard, etc.).

    📶 Wi-Fi Scanner: Real-time detection of surrounding wireless networks.

    🔔 System Notifications: Visual alerts for successful connections or sudden disconnections.

    🛠️ Integrated Tools: Quick access to the system's graphical connection editor.

    🌑 Clean UI: Redirects nmcli output to ensure the TUI remains flicker-free and professional.

🛠️ Prerequisites

To run this tool, ensure the following components are installed on your Linux system:

    NetworkManager (nmcli)

    libnotify (notify-send for system alerts)

    nm-connection-editor (for the graphical "Add VPN" feature)

⌨️ Keyboard Shortcuts
Key	Action
TAB	Switch between VPN and Wi-Fi modes
G	Cycle through available interfaces on the graph
A	Open Connection Editor (Add connection)
ENTER	Connect to selected item (triggers password prompt)
X	Disconnect the selected VPN
R	Manual refresh of all lists
Q	Quit application
🚀 Installation

    Clone the repository:
    Bash

git clone https://github.com/Eristox/DashNet-NetMonitor-TUI.git 
cd cd DashNet-NetMonitor-TUI

Build the project:
Bash

cargo build --release

Run the application:
Bash

    ./target/release/net-monitor-tui

🏗️ Project Structure

    main.rs: Core UI logic (built with Ratatui) and input handling.

    net_monitor.rs: Data fetching module parsing stats from /proc/net/dev.

Developed with ❤️ in Rust.

## ⚖️ License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for more details.
