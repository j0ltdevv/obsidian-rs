//! CLI compagnon d'obsidian-rs : commandes rapides sur un vault depuis le
//! terminal (`obsidian-cli notes|grep|stats <vault>`).
//!
//! Exemples :
//! ```sh
//! obsidian-cli notes demo-vault
//! obsidian-cli grep demo-vault "TODO"
//! obsidian-cli stats demo-vault
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> ! {
    eprintln!("Usage : obsidian-cli <notes|grep|stats> <vault> [motif]");
    std::process::exit(2);
}

fn cmd_notes(vault: &std::path::Path) -> ExitCode {
    match obsidian_rs::vault::scan_vault(vault) {
        Ok(notes) => {
            for note in &notes {
                println!("{}\t{}", note.path.display(), note.name);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Erreur lors du scan : {error}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_grep(vault: &std::path::Path, pattern: &str) -> ExitCode {
    let Ok(notes) = obsidian_rs::vault::scan_vault(vault) else {
        eprintln!("Impossible de scanner le vault.");
        return ExitCode::FAILURE;
    };
    let mut found = 0;
    for note in &notes {
        if let Ok(content) = std::fs::read_to_string(vault.join(&note.path)) {
            for (line_no, line) in content.lines().enumerate() {
                if line.contains(pattern) {
                    println!("{}:{}: {}", note.path.display(), line_no + 1, line);
                    found += 1;
                }
            }
        }
    }
    if found == 0 {
        println!("Aucune correspondance.");
    }
    ExitCode::SUCCESS
}

fn cmd_stats(vault: &std::path::Path) -> ExitCode {
    match obsidian_rs::vault::scan_vault(vault) {
        Ok(notes) => {
            let mut total_lines = 0usize;
            let mut tags: std::collections::BTreeMap<String, usize> = Default::default();
            for note in &notes {
                if let Ok(content) = std::fs::read_to_string(vault.join(&note.path)) {
                    total_lines += content.lines().count();
                }
                for tag in &note.metadata.tags {
                    *tags.entry(tag.clone()).or_default() += 1;
                }
            }
            println!("notes : {}", notes.len());
            println!("lignes : {total_lines}");
            println!("tags : {}", tags.len());
            for (tag, count) in tags.iter().take(10) {
                println!("  #{tag} : {count}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Erreur lors du scan : {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        usage();
    };
    let Some(vault) = args.next().map(PathBuf::from) else {
        usage();
    };
    match command.as_str() {
        "notes" => cmd_notes(&vault),
        "grep" => {
            let Some(pattern) = args.next() else {
                usage();
            };
            cmd_grep(&vault, &pattern)
        }
        "stats" => cmd_stats(&vault),
        _ => usage(),
    }
}
