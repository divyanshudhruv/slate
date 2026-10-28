//! Guards the house style: plain ASCII punctuation in docs and comments.
//! Use `-` for a dash and `->` for an arrow, never the typographic characters.

use std::path::{Path, PathBuf};

/// The characters a comment must not carry, with the name used in the failure message.
const BANNED: &[(&str, &str)] = &[
    ("\u{2014}", "em dash"),
    ("\u{2192}", "right arrow"),
    ("\u{2190}", "left arrow"),
];

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn docs_and_comments_stay_ascii() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(
        !files.is_empty(),
        "no sources found under {}",
        root.display()
    );

    let mut problems = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for (index, line) in text.lines().enumerate() {
            for (bad, name) in BANNED {
                if line.contains(bad) {
                    problems.push(format!(
                        "{}:{}: {name}, use plain ASCII punctuation",
                        path.display(),
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
