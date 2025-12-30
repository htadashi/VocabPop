use clap::Parser;
use rand::seq::SliceRandom;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

#[derive(Debug)]
struct Entry {
    word: String,
    reading: Option<String>,
    meaning: Option<String>,
    codes: Option<String>,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Japanese vocabulary notifier in Rust", long_about = None)]
struct Args {
    /// Vocab directory (text files, one entry per line)
    #[arg(short, long, default_value = "vocab")]
    dir: PathBuf,

    /// Interval in minutes between notifications
    #[arg(short, long, default_value_t = 1)]
    interval: u64,

    /// Show a single notification immediately and exit
    #[arg(long, default_value_t = false)]
    force: bool,

    /// Shuffle vocab entries
    #[arg(long, default_value_t = true)]
    shuffle: bool,
}

fn parse_vocab_file(path: &PathBuf) -> Vec<Entry> {
    let mut out = Vec::new();
    let text = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return out,
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // support tab-separated: word[TAB]reading[TAB]meaning[TAB]codes
        let parts: Vec<&str> = line.split('\t').collect();
        let word = parts.get(0).map(|s| s.trim()).unwrap_or("").to_string();
        if word.is_empty() {
            continue;
        }
        let reading = parts.get(1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let meaning = parts.get(2).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let codes = parts.get(3).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        out.push(Entry { word, reading, meaning, codes });
    }
    out
}

fn load_vocab(dir: &PathBuf) -> Vec<Entry> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let p = entry.path();
            if p.is_file() {
                let mut v = parse_vocab_file(&p);
                entries.append(&mut v);
            }
        }
    }
    entries
}

fn show_notification(title: &str, body: &str) {
    // Try Windows toast via `winrt-notification`. If it fails at runtime, fall back to console.
    #[cfg(target_os = "windows")]
    {
        use winrt_notification::{Sound, Toast};
        let res = Toast::new(Toast::POWERSHELL_APP_ID)
            .title(title)
            .text1(body)
            .sound(Some(Sound::Default))
            .show();
        if let Err(e) = res {
            eprintln!("notification error: {}", e);
            println!("{}\n{}", title, body);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        println!("{}\n{}", title, body);
    }
}

fn main() {
    let args = Args::parse();

    let mut entries = load_vocab(&args.dir);
    if entries.is_empty() {
        eprintln!("No vocab entries found in {:?}. Create text files under that directory.", args.dir);
        return;
    }

    if args.shuffle {
        let mut rng = rand::thread_rng();
        entries.shuffle(&mut rng);
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .ok();

    // channel for tray "Show Now" triggers
    let (tx, rx) = mpsc::channel::<()>();

    // Setup tray icon on Windows. Menu: Show Now, Quit
    #[cfg(target_os = "windows")]
    {
        use tray_icon::{TrayIcon, menu::Menu, menu::MenuItem};
        let tx_clone = tx.clone();
        let running_clone = running.clone();
        std::thread::spawn(move || {
            let mut menu = Menu::new();
            let show_now = MenuItem::new("Show now", true, false);
            let quit = MenuItem::new("Quit", true, false);
            let _ = menu.append(&show_now);
            let _ = menu.append(&quit);

            let mut tray = match TrayIcon::new(None, None, Some(menu)) {
                Ok(t) => t,
                Err(e) => { eprintln!("tray init error: {}", e); return; }
            };

            let tx_show = tx_clone.clone();
            let _ = tray.set_menu(&Box::new(Menu::new()));
            
            // keep thread alive to process tray events
            loop {
                std::thread::sleep(Duration::from_secs(60));
                if !running_clone.load(Ordering::SeqCst) { break; }
            }
        });
    }

    let mut idx = 0usize;

    if args.force {
        let e = &entries[idx % entries.len()];
        let title = &e.word;
        let mut body = String::new();
        if let Some(r) = &e.reading { body.push_str(r); }
        if let Some(m) = &e.meaning { if !body.is_empty() { body.push_str(" — "); } body.push_str(m); }
        if let Some(c) = &e.codes { if !c.is_empty() { body.push_str(" (" ); body.push_str(c); body.push_str(")"); } }
        show_notification(title, &body);
        return;
    }

    let interval = Duration::from_secs(args.interval * 60);
    while running.load(Ordering::SeqCst) {
        // If we received a "show now" from tray, show immediately
        if let Ok(_) = rx.try_recv() {
            let e = &entries[idx % entries.len()];
            let title = &e.word;
            let mut body = String::new();
            if let Some(r) = &e.reading { body.push_str(r); }
            if let Some(m) = &e.meaning { if !body.is_empty() { body.push_str(" — "); } body.push_str(m); }
            if let Some(c) = &e.codes { if !c.is_empty() { body.push_str(" ("); body.push_str(c); body.push_str(")"); } }
            show_notification(title, &body);
            idx = idx.wrapping_add(1);
        } else {
            let e = &entries[idx % entries.len()];
            let title = &e.word;
            let mut body = String::new();
            if let Some(r) = &e.reading { body.push_str(r); }
            if let Some(m) = &e.meaning { if !body.is_empty() { body.push_str(" — "); } body.push_str(m); }
            if let Some(c) = &e.codes { if !c.is_empty() { body.push_str(" ("); body.push_str(c); body.push_str(")"); } }
            show_notification(title, &body);
            idx = idx.wrapping_add(1);
            let mut slept = 0u64;
            while slept < interval.as_secs() && running.load(Ordering::SeqCst) {
                // allow immediate show triggers while sleeping
                if let Ok(_) = rx.try_recv() {
                    break; // break sleep and show immediately next loop
                }
                thread::sleep(Duration::from_secs(1));
                slept += 1;
            }
        }
    }
    println!("Exiting VocabPop.");
}
