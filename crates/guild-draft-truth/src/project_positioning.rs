use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::util::{read_to_string, repo_root};

const POSITIONING_DOC: &str = "docs/project-positioning.md";
const EPIC_DOC: &str = "docs/roadmap/epics/portable-skill-receipts-and-reference-apps.md";

const CHECKED_DOCS: &[&str] = &[
    "README.md",
    "ARCHITECTURE.md",
    "SPECS.md",
    "docs/testing.md",
    "docs/command-language.md",
    "docs/how-guild-works.md",
    "docs/contracts.md",
    "docs/architecture.md",
    "docs/adr/README.md",
    "docs/roadmap.md",
    POSITIONING_DOC,
    EPIC_DOC,
    "examples/README.md",
    "examples/skills/guild-ops-starter/README.md",
];

const POSITIONING_MARKERS: &[&str] = &[
    "## Project Thesis",
    "## Product Thesis",
    "## First Operator Starter Set Thesis",
    "## Preferred Core Terms",
    "## Terms To Avoid As Primary Framing",
    "## Sane Defaults",
    "## Sane Assumptions",
    "## Sane Expectations",
    "## Sane Implementations",
    "## Boundary",
];

const POSITIONING_PHRASES: &[&str] = &[
    "Guild is trusted operational automation for engineering teams.",
    "The playbook is the application. The trust chain is the product.",
    "Guild Ops Starter is the first operator starter set in the repo. It is a repo-local release slice built on that trust chain. It uses receipts and evidence to summarize incidents, compare runs, explain evidence, and generate bounded operational reports without pretending it is the whole product.",
];

const REQUIRED_RAW_SNIPPETS: &[(&str, &[&str])] = &[
    (
        "README.md",
        &[
            "docs/project-positioning.md",
            "docs/roadmap/epics/portable-skill-receipts-and-reference-apps.md",
        ],
    ),
    ("ARCHITECTURE.md", &["docs/project-positioning.md"]),
    ("SPECS.md", &["docs/project-positioning.md"]),
    (
        "docs/testing.md",
        &[
            "project-positioning.md",
            "cargo run -q -p xtask -- project-positioning check",
        ],
    ),
    ("docs/command-language.md", &["project-positioning.md"]),
    ("docs/how-guild-works.md", &["project-positioning.md"]),
    ("docs/contracts.md", &["project-positioning.md"]),
    ("docs/architecture.md", &["project-positioning.md"]),
    ("docs/adr/README.md", &["../project-positioning.md"]),
    (
        "docs/roadmap.md",
        &[
            "project-positioning.md",
            "roadmap/epics/portable-skill-receipts-and-reference-apps.md",
        ],
    ),
    (
        POSITIONING_DOC,
        &["roadmap/epics/portable-skill-receipts-and-reference-apps.md"],
    ),
    (EPIC_DOC, &["../../project-positioning.md"]),
    ("examples/README.md", &["../docs/project-positioning.md"]),
    (
        "examples/skills/guild-ops-starter/README.md",
        &["repo-local release slice"],
    ),
];

const REQUIRED_NORMALIZED_SNIPPETS: &[(&str, &[&str])] = &[
    (POSITIONING_DOC, POSITIONING_PHRASES),
    (
        "README.md",
        &[
            "Guild is trusted operational automation for engineering teams.",
            "Today, the repo exposes that model through portable skills, bounded capabilities, durable execution and evidence records, and stable Guild refs.",
            "Guild Ops Starter is the first operator starter set in the repo. It is a repo-local release slice built on that trust chain, not the whole product story.",
            "bounded live-proof coverage for specific `read-resource`, `http-request`, `invoke-skill`, `emit-evidence`, and `log-write` slices",
        ],
    ),
    (
        EPIC_DOC,
        &[
            "Guild is trusted operational automation for engineering teams.",
            "The playbook is the application. The trust chain is the product.",
            "eight `http-request`, two `invoke-skill`, and one exact `emit-evidence` checked slices, plus proof-only `log-write`",
            "Guild Ops Starter clearly reads as the first operator starter set and a repo-local release slice, not the whole product thesis",
        ],
    ),
    (
        "examples/skills/guild-ops-starter/README.md",
        &[
            "Guild Ops Starter is the first operator starter set in the repo. It is a repo-local release slice built on that trust chain, not the whole product story.",
        ],
    ),
];

const INTRO_FORBIDDEN_PHRASES: &[(&str, usize, &[&str])] = &[
    ("README.md", 12, &["runtime and control plane"]),
    ("ARCHITECTURE.md", 20, &["skill execution fabric"]),
    ("docs/how-guild-works.md", 8, &["platform contract"]),
    (
        "examples/skills/guild-ops-starter/README.md",
        10,
        &["starter pack"],
    ),
];

pub fn check() -> Result<()> {
    let documents = load_documents()?;

    let positioning = document(&documents, POSITIONING_DOC)?;
    ensure_contains_all_raw(positioning, POSITIONING_MARKERS)?;
    ensure_contains_all_normalized(positioning, POSITIONING_PHRASES)?;

    for (document_path, required_snippets) in REQUIRED_RAW_SNIPPETS {
        let document = document(&documents, document_path)?;
        ensure_contains_all_raw(document, required_snippets)?;
    }

    for (document_path, required_snippets) in REQUIRED_NORMALIZED_SNIPPETS {
        let document = document(&documents, document_path)?;
        ensure_contains_all_normalized(document, required_snippets)?;
    }

    for (document_path, max_lines, forbidden_phrases) in INTRO_FORBIDDEN_PHRASES {
        let document = document(&documents, document_path)?;
        ensure_intro_excludes(document, *max_lines, forbidden_phrases)?;
    }

    println!("project positioning validates cleanly.");
    Ok(())
}

struct LoadedDocument {
    path: PathBuf,
    text: String,
    normalized_text: String,
}

fn load_documents() -> Result<BTreeMap<&'static str, LoadedDocument>> {
    let mut documents = BTreeMap::new();
    for relative_path in CHECKED_DOCS {
        let path = repo_root().join(relative_path);
        if !path.exists() {
            bail!(
                "required project-positioning document `{relative_path}` is missing at {}",
                path.display()
            );
        }
        let text = read_to_string(&path)?;
        validate_markdown_links(&path, &text)?;
        documents.insert(
            *relative_path,
            LoadedDocument {
                path,
                normalized_text: normalize_whitespace(&text),
                text,
            },
        );
    }
    Ok(documents)
}

fn document<'a>(
    documents: &'a BTreeMap<&'static str, LoadedDocument>,
    relative_path: &str,
) -> Result<&'a LoadedDocument> {
    documents.get(relative_path).with_context(|| {
        format!("project-positioning check is missing loaded document `{relative_path}`")
    })
}

fn ensure_contains_all_raw(document: &LoadedDocument, required_snippets: &[&str]) -> Result<()> {
    for snippet in required_snippets {
        if !document.text.contains(snippet) {
            bail!(
                "project-positioning document `{}` is missing required snippet `{snippet}`",
                display_relative_path(document)
            );
        }
    }
    Ok(())
}

fn ensure_contains_all_normalized(
    document: &LoadedDocument,
    required_snippets: &[&str],
) -> Result<()> {
    for snippet in required_snippets {
        let normalized_snippet = normalize_whitespace(snippet);
        if !document.normalized_text.contains(&normalized_snippet) {
            bail!(
                "project-positioning document `{}` is missing required wording `{snippet}`",
                display_relative_path(document)
            );
        }
    }
    Ok(())
}

fn ensure_intro_excludes(
    document: &LoadedDocument,
    max_lines: usize,
    forbidden_phrases: &[&str],
) -> Result<()> {
    let intro = document
        .text
        .lines()
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n");
    let normalized_intro = normalize_whitespace(&intro).to_lowercase();
    for phrase in forbidden_phrases {
        if normalized_intro.contains(&normalize_whitespace(phrase).to_lowercase()) {
            bail!(
                "project-positioning intro drifted in `{}`: found forbidden phrase `{phrase}` in the first {max_lines} lines",
                display_relative_path(document)
            );
        }
    }
    Ok(())
}

fn validate_markdown_links(document_path: &Path, text: &str) -> Result<()> {
    for link in extract_markdown_links(text) {
        let Some(target) = resolve_local_markdown_link(document_path, &link) else {
            continue;
        };
        if !target.exists() {
            bail!(
                "project-positioning markdown link `{link}` in {} does not resolve to a real file",
                document_path.display()
            );
        }
    }
    Ok(())
}

fn extract_markdown_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("](") {
        let after_marker = &remaining[start + 2..];
        let Some(end) = after_marker.find(')') else {
            break;
        };
        links.push(after_marker[..end].to_owned());
        remaining = &after_marker[end + 1..];
    }
    links
}

fn resolve_local_markdown_link(document_path: &Path, link: &str) -> Option<PathBuf> {
    let destination = markdown_link_destination(link)?;
    if destination.starts_with('#')
        || destination.starts_with("mailto:")
        || destination.contains("://")
    {
        return None;
    }
    let path_part = destination.split('#').next().unwrap_or(destination);
    if path_part.is_empty() {
        return None;
    }
    Some(
        document_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path_part),
    )
}

fn markdown_link_destination(link: &str) -> Option<&str> {
    let trimmed = link.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('<') {
        let destination = rest.split('>').next().unwrap_or(rest).trim();
        return (!destination.is_empty()).then_some(destination);
    }
    trimmed.split_whitespace().next()
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn display_relative_path(document: &LoadedDocument) -> String {
    document
        .path
        .strip_prefix(repo_root())
        .unwrap_or(&document.path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{markdown_link_destination, resolve_local_markdown_link};

    #[test]
    fn markdown_link_destination_strips_optional_titles() {
        assert_eq!(
            markdown_link_destination(r#"project-positioning.md "Project framing""#),
            Some("project-positioning.md")
        );
        assert_eq!(
            markdown_link_destination(r#"project-positioning.md#boundary "Project framing""#),
            Some("project-positioning.md#boundary")
        );
    }

    #[test]
    fn markdown_link_destination_supports_angle_bracket_destinations() {
        assert_eq!(
            markdown_link_destination(r#"<project-positioning.md#boundary> "Project framing""#),
            Some("project-positioning.md#boundary")
        );
    }

    #[test]
    fn resolve_local_markdown_link_uses_only_the_destination_path() {
        let resolved = resolve_local_markdown_link(
            Path::new("docs/roadmap.md"),
            r#"project-positioning.md "Project framing""#,
        )
        .expect("titled local links should resolve");

        assert_eq!(resolved, Path::new("docs").join("project-positioning.md"));
    }
}
