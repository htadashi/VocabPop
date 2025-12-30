# VocabPop

Japanese vocabulary notifier written in Rust.
Inspired in [https://www.tanos.co.uk/jlpt/skills/vocab/vocabbubble/](VocabBubble)

Features:
- Reads text files from the `vocab` directory (tab-separated lines).
- Shows a notification every N minutes (default 1).
- Can shuffle entries and force a single notification.

Build (Windows):

```powershell
cargo build --release
```

Run:

```powershell
# run background notifier
cargo run -- --dir vocab --interval 1

# show a single notification and exit
cargo run -- --force
```

Notes:
- On Windows the project tries to use `winrt-notification` for native toasts. If that fails, the program prints entries to the console.
- This is a starting point — adding a system tray icon and persistent settings is left as an enhancement.
