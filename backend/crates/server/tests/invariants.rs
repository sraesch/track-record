//! Architecture invariants from CLAUDE.md that the compiler cannot check,
//! and the committed OpenAPI documents. No database needed.

use std::path::{Path, PathBuf};

fn sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    assert!(out.len() > 5);
    out
}

/// True if `word` occurs as a whole identifier (so `get_series_points` does
/// not count as `series_point`).
fn mentions_word(text: &str, word: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

#[test]
fn word_matching() {
    assert!(mentions_word("FROM series_point WHERE", "series_point"));
    assert!(!mentions_word("get_series_points()", "series_point"));
}

/// Only the series store touches the points table (ADR 0002, NFR-4).
#[test]
fn only_the_series_store_touches_points() {
    for (path, text) in sources() {
        if path.ends_with("series_store.rs") {
            continue;
        }
        assert!(
            !mentions_word(&text, "series_point"),
            "{} uses series_point",
            path.display()
        );
    }
}

/// Only the attachment store touches files (ADR 0003).
#[test]
fn only_the_attachment_store_touches_files() {
    for (path, text) in sources() {
        if path.ends_with("attachment_store.rs") {
            continue;
        }
        for forbidden in ["std::fs", "tokio::fs", "File::open", "File::create"] {
            assert!(
                !text.contains(forbidden),
                "{} uses {forbidden}",
                path.display()
            );
        }
    }
}

/// The committed OpenAPI documents (used for frontend type generation)
/// match the code. Regenerate with:
///   cargo run -p track-record-server -- openapi machine > openapi/machine.json
///   cargo run -p track-record-server -- openapi ui > openapi/ui.json
#[tokio::test]
async fn committed_openapi_documents_are_current() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../openapi");
    for which in ["machine", "ui"] {
        let generated = track_record_server::openapi_json(which).unwrap();
        let path = dir.join(format!("{which}.json"));
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed.trim_end() == generated.trim_end(),
            "{} is out of date; regenerate it (see this test's doc comment)",
            path.display()
        );
    }
}
