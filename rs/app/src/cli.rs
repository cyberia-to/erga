//! The terminal face of erga.
//!
//! Same binary as the window, because the command a person types should be
//! the name of the thing. Everything here is meant to be read at a glance:
//! one mark, one column of keys, one column of values.

use crate::{pool, pools, store};

const MARK: &str = "\x1b[38;5;121m";
const DIM: &str = "\x1b[38;5;244m";
const WARM: &str = "\x1b[38;5;215m";
const OFF: &str = "\x1b[0m";

pub fn help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "\n{MARK}  ⬡  erga{OFF}  {DIM}v{v} · one-button ERGO miner for Apple Silicon{OFF}\n"
    );
    for (cmd, what) in [
        ("erga", "open the window"),
        ("erga mine [host] [port] [address]", "mine here, no window"),
        ("erga status", "what the pool owes you"),
        ("erga link", "put `erga` on your PATH"),
        ("erga help", "this"),
    ] {
        println!("  {MARK}{cmd:<34}{OFF}{DIM}{what}{OFF}");
    }
    println!(
        "\n  {DIM}mine, with nothing after it, uses the pool you picked in the window\n  \
         and the wallet it generated. every share is re-verified on the CPU\n  \
         reference before it is sent.{OFF}\n"
    );
}

fn row(key: &str, val: &str) {
    println!("  {DIM}{key:<22}{OFF}{val}");
}

/// What the pool has actually credited, from the terminal.
pub fn status() {
    let idx = pools::load_choice();
    let pl = pools::get(idx);
    let addr = match erga_wallet::Wallet::load_or_create() {
        Ok(w) => w.address,
        Err(e) => {
            eprintln!("no wallet: {e}");
            std::process::exit(1);
        }
    };
    println!("\n{MARK}  ⬡  erga{OFF}  {DIM}{}{OFF}\n", pl.label);
    let p = pool::snapshot(&addr, idx);
    row("address", &addr);
    if let Some(e) = &p.error {
        println!("  {WARM}{e}{OFF}\n");
        return;
    }
    let earned = p.balance_erg + p.pending_erg;
    let pct = (earned / p.threshold_erg * 100.0).min(100.0);
    row("credited", &format!("{:.5} ERG", p.balance_erg.max(0.0)));
    row("maturing", &format!("{:.5} ERG", p.pending_erg.max(0.0)));
    if p.paid_erg > 0.0 {
        row("paid out", &format!("{:.5} ERG", p.paid_erg));
    }
    row(
        "toward payout",
        &format!("{pct:.1}%  of {:.1} ERG", p.threshold_erg),
    );
    row("pool sees", &format!("{:.0} MH/s  (24h)", p.hashrate_24h_mhs));
    if p.difficulty > 0.0 && p.hashrate_24h_mhs > 0.01 {
        let net = p.difficulty / pool::BLOCK_TIME_S;
        let per_day = p.hashrate_24h_mhs * 1e6 / net * (86_400.0 / pool::BLOCK_TIME_S)
            * pool::BLOCK_REWARD_ERG;
        let usd = if p.price_usd > 0.0 {
            format!("  ·  ${:.2}", per_day * 30.0 * p.price_usd)
        } else {
            String::new()
        };
        row("a month at this pace", &format!("≈ {:.2} ERG{usd}", per_day * 30.0));
    }
    println!();
}

/// Put `erga` where a shell will find it. The app lives in a bundle, so the
/// binary inside it is what gets linked — the link keeps working across
/// upgrades because the path inside the bundle does not change.
pub fn link() {
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("cannot find my own path");
        std::process::exit(1);
    };
    match place_link(&exe) {
        Some(p) => {
            println!("\n  {MARK}linked{OFF}  {}\n", p.display());
            if !p.starts_with("/usr/local") {
                println!(
                    "  {DIM}add it to your PATH if it is not already:{OFF}\n  \
                     {DIM}  export PATH=\"$HOME/.local/bin:$PATH\"{OFF}\n"
                );
            }
        }
        None => {
            eprintln!(
                "\n  could not write to /usr/local/bin or ~/.local/bin.\n  \
                 link it yourself:\n    ln -sf {} /usr/local/bin/erga\n",
                exe.display()
            );
            std::process::exit(1);
        }
    }
}

/// Try the usual places, in order of how likely a shell is to look there.
/// Returns where the link landed. Silent on failure — called at startup too.
pub fn place_link(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let mut targets = vec![std::path::PathBuf::from("/usr/local/bin")];
    if let Some(h) = &home {
        targets.push(h.join(".local/bin"));
    }
    for dir in targets {
        if !dir.exists() && std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let link = dir.join("erga");
        if std::fs::read_link(&link).is_ok_and(|t| t == exe) {
            return Some(link); // already right
        }
        let _ = std::fs::remove_file(&link);
        if std::os::unix::fs::symlink(exe, &link).is_ok() {
            return Some(link);
        }
    }
    None
}

/// Link on launch when it costs nothing — no prompt, no admin, no surprise.
/// A bundle the user dragged to /Applications should simply work in a shell.
pub fn link_quietly() {
    if let Ok(exe) = std::env::current_exe() {
        // only from inside a bundle: a dev build lives in target/ and would
        // leave a link pointing at a path that gets rebuilt under it
        if exe.to_string_lossy().contains(".app/Contents/MacOS/") {
            if let Some(p) = place_link(&exe) {
                store::log(&format!("cli linked at {}", p.display()));
            }
        }
    }
}
