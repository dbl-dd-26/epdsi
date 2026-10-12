#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Plain `std` file reads, so these run identically under
//! `cargo test` and `cargo test --no-default-features --features graphics`.

//! Guards the published metadata against drift: every driver IC the crate ships must be named in
//! the `Cargo.toml` description, and the crate-level "N panels" tagline must match the panel
//! files. The description is baked into each published version, so a miss here costs a patch
//! release (it has been missed twice: JD79676/IST7163, then SSD1685).
//!
//! The expected names are read from `src/controllers`, not listed here, so adding a controller
//! or a variant without touching the description fails this test.

use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// The `description = "..."` value of `Cargo.toml`.
fn description() -> String {
    let manifest = read("Cargo.toml");
    let line = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("description"))
        .expect("Cargo.toml has no description");
    let start = line.find('"').expect("description is not quoted") + 1;
    let end = line.rfind('"').expect("description is not quoted");
    line[start..end].to_string()
}

/// Variant names of `pub enum <name>` in `source`, one variant per line at four-space indent.
fn variants_of(source: &str, name: &str) -> Vec<String> {
    let header = format!("pub enum {name} {{");
    let body = source
        .split(&header)
        .nth(1)
        .unwrap_or_else(|| panic!("no `{header}` found"));
    let body = &body[..body.find("\n}").expect("enum is not closed")];
    body.lines()
        .filter_map(|l| {
            let l = l.strip_prefix("    ")?;
            if l.starts_with(' ') || l.starts_with('/') || l.starts_with('#') {
                return None;
            }
            let ident = l.strip_suffix(',')?;
            ident
                .chars()
                .all(|c| c.is_ascii_alphanumeric())
                .then(|| ident.to_string())
        })
        .collect()
}

/// Every driver IC name the crate ships, upper-cased as written in the description, plus the
/// `"Pervasive"` marker for the two Pervasive Displays controllers.
fn shipped_ics() -> Vec<String> {
    let dir = root().join("src/controllers");
    let mut ics: Vec<String> = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();

        // `pub struct Ist7163Controller` and friends: one IC per dedicated controller type.
        for line in source.lines() {
            let Some(rest) = line.strip_prefix("pub struct ") else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            let Some(stem) = name.strip_suffix("Controller") else {
                continue;
            };
            if stem.starts_with("Pervasive") {
                ics.push("Pervasive".to_string());
            } else if !stem.ends_with('x') {
                // `Ssd168xController` and `Jd7966xController` are family names, not part numbers;
                // their variants are read below.
                ics.push(stem.to_uppercase());
            }
        }

        // Family controllers name their ICs as enum variants (`Ssd1685` has no struct of its own).
        for family in ["Ssd168xVariant", "Jd7966xVariant"] {
            if source.contains(&format!("pub enum {family} {{")) {
                ics.extend(
                    variants_of(&source, family)
                        .iter()
                        .map(|v| v.to_uppercase()),
                );
            }
        }
    }
    ics.sort();
    ics.dedup();
    ics
}

#[test]
fn test_shipped_ic_list_is_not_empty_and_covers_known_families() {
    // Sanity check on the scanner itself, so a parsing change cannot turn the real guard below
    // into a silent no-op.
    let ics = shipped_ics();
    for expected in [
        "SSD1680",
        "SSD1685",
        "JD79676",
        "IST7163",
        "UC8253",
        "Pervasive",
    ] {
        assert!(
            ics.iter().any(|i| i == expected),
            "scanner missed {expected}; found {ics:?}"
        );
    }
}

#[test]
fn test_description_names_every_shipped_ic() {
    let description = description();
    let missing: Vec<String> = shipped_ics()
        .into_iter()
        .filter(|ic| !description.contains(ic.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "Cargo.toml description does not mention {missing:?}.\n\
         The description is published with each version; add the IC(s) before tagging.\n\
         description = {description:?}"
    );
}

const NUMBER_WORDS: [&str; 31] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "twenty-one",
    "twenty-two",
    "twenty-three",
    "twenty-four",
    "twenty-five",
    "twenty-six",
    "twenty-seven",
    "twenty-eight",
    "twenty-nine",
    "thirty",
];

#[test]
fn test_crate_tagline_panel_count_matches_panel_files() {
    // "covering twelve controllers and seventeen panels" in the crate-level docs.
    let lib = read("src/lib.rs");
    let tagline = lib
        .lines()
        .find(|l| l.contains("controllers and") && l.contains("panels"))
        .expect("no `covering N controllers and M panels` line in src/lib.rs");
    let after = tagline.split("controllers and").nth(1).unwrap().trim();
    let word = after.split_whitespace().next().unwrap();
    let stated = NUMBER_WORDS
        .iter()
        .position(|w| *w == word)
        .unwrap_or_else(|| panic!("unrecognised number word {word:?} in: {tagline}"));

    let files = fs::read_dir(root().join("src/panels"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rs") && n != "mod.rs")
        .count();

    assert_eq!(
        stated, files,
        "src/lib.rs says {stated} panels but src/panels has {files} panel files"
    );
}
