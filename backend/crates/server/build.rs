//! Embeds the list of database migrations (db/migrations/NNNN_name.sql) so
//! the server can refuse to start against a database that lacks one.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../../db/migrations");
    println!("cargo::rerun-if-changed={}", dir.display());

    let mut migrations: Vec<(i32, String)> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".sql")?;
            let (version, name) = stem.split_once('_')?;
            Some((version.parse().ok()?, name.to_owned()))
        })
        .collect();
    migrations.sort();

    let body: Vec<String> = migrations
        .iter()
        .map(|(v, name)| format!("({v}, {name:?})"))
        .collect();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("migrations.rs");
    fs::write(out, format!("&[{}]", body.join(", "))).unwrap();
}
