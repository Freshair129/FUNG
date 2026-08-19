use genesis_block_native::{OpenOptions, Storage};
use std::{env, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "health" => {
            let db_path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("genesisdb"));
            match Storage::open(OpenOptions {
                path: db_path.display().to_string(),
                page_cache_mb: Some(16),
                read_only: Some(false),
                vector_dim: Some(384),
            }) {
                Ok(storage) => {
                    println!(
                        "{{\"app\":\"FUNG\",\"databasePath\":\"{}\",\"storageAuthority\":\"GenesisBlockDB signed WAL\",\"stableFrontier\":{}}}",
                        db_path.display(),
                        storage.stable_frontier()
                    );
                }
                Err(error) => {
                    eprintln!("failed to open GenesisBlockDB: {error}");
                    process::exit(1);
                }
            }
        }
        "help" | "--help" | "-h" => {
            println!("FUNG CLI");
            println!("Usage:");
            println!("  fung-cli health [genesis-path]");
        }
        other => {
            eprintln!("unknown command: {other}");
            process::exit(2);
        }
    }
}
