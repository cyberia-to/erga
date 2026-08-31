//! The terminal face of erga.
//!
//! Same binary as the window, because the command a person types should be
//! the name of the thing. Everything here is meant to be read at a glance:
//! one mark, one column of keys, one column of values.

use erga_app::{pool, pools, store};

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
        ("erg", "open the window"),
        ("erg mine [host] [port] [address]", "mine here, no window"),
        ("erg mine --intensity max|eco|min", "how hard to push"),
        ("erg status", "what the pool owes you"),
        ("erg link", "put the command on your PATH"),
        ("erg difftest", "prove the GPU kernel against the reference"),
        ("erg buildbench [height]", "time one epoch-table build"),
        ("erg help", "this"),
    ] {
        println!("  {MARK}{cmd:<34}{OFF}{DIM}{what}{OFF}");
    }
    println!("\n  {DIM}`erga` and `erg` are the same command.{OFF}");
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
    let addr = match erga_app::payout_address() {
        Ok(a) => a,
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
        Some(l) => {
            println!("\n  {MARK}linked{OFF}  {}  {DIM}(and erg){OFF}\n", l.path.display());
            if !l.on_path {
                // Say the actual directory, not a guess: the point of the
                // line is that it can be pasted.
                let dir = l.path.parent().unwrap_or(&l.path).display();
                println!(
                    "  {DIM}that directory is not on your PATH yet:{OFF}\n  \
                     {DIM}  export PATH=\"{dir}:$PATH\"{OFF}\n"
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

/// Both names the command answers to: the full one, and a short one for the
/// hand that types it every day.
pub const NAMES: [&str; 2] = ["erga", "erg"];

/// Where a link landed, and whether a shell will actually find it there.
pub struct Linked {
    pub path: std::path::PathBuf,
    pub on_path: bool,
}

/// The directories this shell searches, from $PATH.
fn path_dirs() -> Vec<std::path::PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

/// Try the places a shell looks, preferring ones already on $PATH.
///
/// Preferring a fixed list instead once put the link in ~/.local/bin on a
/// machine whose shells had never heard of that directory: `erga` reported
/// itself linked and then was not a command. A link outside $PATH is a link
/// nobody can type.
///
/// Returns where the link landed. Silent on failure — called at startup too.
pub fn place_link(exe: &std::path::Path) -> Option<Linked> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let on_path = path_dirs();
    let is_on_path = |d: &std::path::Path| on_path.iter().any(|p| p == d);

    // /usr/local/bin first when it is writable — the traditional place, and
    // on every shell's PATH. Then whatever the user's own PATH already has:
    // a directory they chose beats one we invent. ~/.local/bin closes the
    // list, created if need be, because somewhere is better than nowhere.
    let mut targets = vec![std::path::PathBuf::from("/usr/local/bin")];
    // The user's own directories before a package manager's prefix: brew
    // objects to unbrewed binaries under /opt/homebrew, and rightly.
    let mine = |d: &std::path::PathBuf| home.as_ref().is_some_and(|h| d.starts_with(h));
    targets.extend(on_path.iter().filter(|d| mine(d)).cloned());
    targets.extend(on_path.iter().filter(|d| !mine(d) && d.starts_with("/opt")).cloned());
    if let Some(h) = &home {
        targets.push(h.join(".local/bin"));
    }

    for dir in targets {
        if !dir.exists() && std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        // The short name is a convenience; the real one decides success.
        let Some(path) = link_as(exe, &dir, NAMES[0]) else { continue };
        for alias in &NAMES[1..] {
            link_as(exe, &dir, alias);
        }
        let on_path = is_on_path(&dir);
        return Some(Linked { path, on_path });
    }
    None
}

/// One symlink, replacing whatever was there. None if the directory refuses.
fn link_as(exe: &std::path::Path, dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let link = dir.join(name);
    if std::fs::read_link(&link).is_ok_and(|t| t == exe) {
        return Some(link); // already right
    }
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(exe, &link).ok().map(|()| link)
}

/// Link on launch when it costs nothing — no prompt, no admin, no surprise.
/// A bundle the user dragged to /Applications should simply work in a shell.
pub fn link_quietly() {
    if let Ok(exe) = std::env::current_exe() {
        // only from inside a bundle: a dev build lives in target/ and would
        // leave a link pointing at a path that gets rebuilt under it
        if exe.to_string_lossy().contains(".app/Contents/MacOS/") {
            if let Some(l) = place_link(&exe) {
                // Log whether a shell can reach it, so a silent link that
                // nobody can type leaves evidence.
                store::log(&format!(
                    "cli linked at {} (on PATH: {})",
                    l.path.display(),
                    l.on_path
                ));
            }
        }
    }
}
