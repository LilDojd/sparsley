//! Keeps the README diagrams in sync with `sparsley_diagram::diagram!`.
//!
//! Run with `BLESS=1` to rewrite stale blocks.

use std::{env, fs};

const LAYOUT: &str = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]);
};

const REMOVE: &str = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]);
    map.remove(3);
};

/// Replaces the fenced block after each marker with its diagram.
fn sync(readme: &str) -> String {
    let mut out = String::new();
    let mut lines = readme.lines();
    while let Some(line) = lines.next() {
        out.push_str(line);
        out.push('\n');
        let Some(name) = line
            .strip_prefix("<!-- diagram: ")
            .and_then(|rest| rest.strip_suffix(" -->"))
        else {
            continue;
        };
        let diagram = match name {
            "layout" => LAYOUT,
            "remove" => REMOVE,
            _ => panic!("unknown diagram `{name}`"),
        };
        assert_eq!(lines.next(), Some("```text"), "block after `{name}`");
        assert!(
            lines.any(|line| line == "```"),
            "unclosed block for `{name}`"
        );
        out.push_str(diagram);
        out.push('\n');
    }
    out
}

#[test]
#[cfg_attr(miri, ignore = "reads README.md")]
fn readme_diagrams_are_fresh() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/README.md");
    let readme = fs::read_to_string(path).unwrap();
    let synced = sync(&readme);
    if env::var_os("BLESS").is_some() {
        fs::write(path, synced).unwrap();
    } else {
        assert!(
            readme == synced,
            "README.md has stale diagrams; rerun with BLESS=1"
        );
    }
}
