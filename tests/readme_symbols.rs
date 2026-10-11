#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Guards the README's Rust snippets against API drift.
//!
//! The README fences are `rust,ignore`: they lean on `spi_device`, `delay` and the like, so they
//! cannot compile as written, and the README is not part of the crate docs. This test reads the
//! README text and checks the names the snippets rely on:
//!
//! - every controller, variant, refresh-mode and panel identifier inside a ```rust fence is
//!   exported from `src/prelude.rs`;
//! - every `epd.<method>(` call names a method declared in `src/driver.rs`.
//!
//! It is a name check, not a type check. A renamed or removed item fails here instead of
//! shipping a stale README to crates.io.

const README: &str = include_str!("../README.md");
const PRELUDE: &str = include_str!("../src/prelude.rs");
const DRIVER: &str = include_str!("../src/driver.rs");

/// Supporting types the snippets use that are not a controller, variant or panel by name.
const KNOWN_TYPES: &[&str] = &[
    "EpdBuilder",
    "EpdDriver",
    "SpiBusWrapper",
    "ColorChannel",
    "ColorMode",
    "SevenColor",
    "PageBuffer",
    "PageBufferPair",
    "GrayBufferPair",
    "Gray4Color",
    "Gray4Polarity",
    "DisplayRotation",
    "TriColor",
];

const PANEL_PREFIXES: &[&str] = &[
    "GDEM", "GDEY", "GDEQ", "GDEP", "SE0352", "ZJY", "GxEPD2_", "EPD_",
];

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Returns `(line_number, text)` for each line inside a ```rust fence.
fn rust_fence_lines() -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    let mut in_rust = false;
    let mut in_other = false;
    for (i, line) in README.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_rust || in_other {
                in_rust = false;
                in_other = false;
            } else if trimmed.starts_with("```rust") {
                in_rust = true;
            } else {
                in_other = true;
            }
            continue;
        }
        if in_rust {
            out.push((i + 1, line));
        }
    }
    out
}

fn identifiers(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        if is_ident_char(c) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.push(&line[s..i]);
        }
    }
    if let Some(s) = start {
        out.push(&line[s..]);
    }
    out
}

/// Skips `//` comments, so prose such as "GDEY0213F51 needs Jd79661Controller" is not checked.
fn code_part(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn contains_word(haystack: &str, word: &str) -> bool {
    identifiers_in_text(haystack).any(|w| w == word)
}

fn identifiers_in_text(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !is_ident_char(c))
        .filter(|s| !s.is_empty())
}

fn looks_like_epdsi_symbol(id: &str) -> bool {
    let first_upper = id.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    if !first_upper {
        return false;
    }
    if KNOWN_TYPES.contains(&id) {
        return true;
    }
    if id.ends_with("Controller") || id.ends_with("Variant") || id.ends_with("RefreshMode") {
        return true;
    }
    if PANEL_PREFIXES.iter().any(|p| id.starts_with(p)) {
        return true;
    }
    // Pervasive part numbers, for example `E2266KS0C1`: E, four digits, then capitals/digits.
    let b = id.as_bytes();
    b.len() >= 8
        && b[0] == b'E'
        && b[1..5].iter().all(u8::is_ascii_digit)
        && id
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

#[test]
fn readme_snippet_symbols_are_exported_from_the_prelude() {
    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (line_no, line) in rust_fence_lines() {
        for id in identifiers(code_part(line)) {
            if !looks_like_epdsi_symbol(id) {
                continue;
            }
            checked += 1;
            if !contains_word(PRELUDE, id) {
                missing.push(format!(
                    "README.md:{line_no}: `{id}` is not exported from the prelude"
                ));
            }
        }
    }
    assert!(
        checked > 40,
        "expected many README symbols, found {checked}: fence parsing broke"
    );
    assert!(
        missing.is_empty(),
        "stale README symbols:\n{}",
        missing.join("\n")
    );
}

#[test]
fn readme_snippet_driver_methods_exist() {
    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (line_no, line) in rust_fence_lines() {
        let code = code_part(line);
        let mut rest = code;
        while let Some(pos) = rest.find("epd.") {
            let before_ok = match rest[..pos].chars().next_back() {
                Some(c) => !is_ident_char(c),
                None => true,
            };
            let after = &rest[pos + 4..];
            let name: String = after.chars().take_while(|c| is_ident_char(*c)).collect();
            if before_ok && after[name.len()..].starts_with('(') {
                checked += 1;
                let declared = DRIVER.lines().any(|l| {
                    let l = l.trim_start();
                    (l.starts_with("pub fn ") || l.starts_with("pub async fn "))
                        && identifiers(l).get(if l.starts_with("pub async") { 3 } else { 2 })
                            == Some(&name.as_str())
                });
                if !declared {
                    missing.push(format!(
                        "README.md:{line_no}: `epd.{name}()` is not in src/driver.rs"
                    ));
                }
            }
            rest = &after[name.len()..];
        }
    }
    assert!(
        checked > 20,
        "expected many `epd.` calls, found {checked}: parsing broke"
    );
    assert!(
        missing.is_empty(),
        "stale README method calls:\n{}",
        missing.join("\n")
    );
}
