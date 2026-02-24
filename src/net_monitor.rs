use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;

pub struct NetStats {
    pub rx: u64,
    pub tx: u64,
}

pub fn get_net_data() -> HashMap<String, NetStats> {
    let mut stats = HashMap::new();
    if let Ok(file) = File::open("/proc/net/dev") {
        let reader = BufReader::new(file);
        for line in reader.lines().skip(2).flatten() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 10 {
                let iface = parts[0].replace(':', "");
                let rx = parts[1].parse::<u64>().unwrap_or(0);
                let tx = parts[9].parse::<u64>().unwrap_or(0);
                stats.insert(iface, NetStats { rx, tx });
            }
        }
    }
    stats
}

