use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::util::{read_to_string, repo_root};

const REQUIRED_DOCS: &[&str] = &[
    "AGENTS.md",
    "README.md",
    "ARCHITECTURE.md",
    "SPECS.md",
    "docs/project-positioning.md",
    "docs/how-guild-works.md",
    "docs/command-language.md",
    "docs/testing.md",
    "docs/roadmap.md",
    "docs/contracts.md",
    "docs/architecture.md",
    "docs/adr/README.md",
    "docs/adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md",
    "docs/strategy/session-substrate/00-umbrella-epic.md",
    "docs/strategy/session-substrate/01-north-star.md",
    "docs/strategy/session-substrate/07-roadmap.md",
    "docs/strategy/session-substrate/tasks.md",
    "docs/strategy/session-substrate/context.yaml",
];

const REQUIRED_SNIPPETS: &[(&str, &[&str])] = &[
    (
        "AGENTS.md",
        &[
            "trusted session substrate for isolated harness execution",
            "session broker",
            "harness",
            "Current Milestone",
            "Next Likely Tasks",
        ],
    ),
    (
        "README.md",
        &[
            "## Current Direction",
            "admission controller, session broker, and receipt engine for isolated harness execution",
            "The product abstraction is the session,",
            "The current live repo still exposes a skill-first, inspect-first trust chain",
        ],
    ),
    (
        "docs/project-positioning.md",
        &[
            "compatibility bridge",
            "session, not the sandbox",
            "What Ships Today",
        ],
    ),
    (
        "docs/strategy/session-substrate/00-umbrella-epic.md",
        &[
            "admission controller, session broker, and receipt engine for isolated harness execution",
            "The product abstraction is the session, not the sandbox",
            "done enough for v1",
        ],
    ),
    (
        "docs/strategy/session-substrate/01-north-star.md",
        &[
            "trusted session substrate for isolated harness execution",
            "Session is the product abstraction",
            "Sandbox lifecycle is internal",
        ],
    ),
    (
        "docs/strategy/session-substrate/07-roadmap.md",
        &[
            "M1 Session Vocabulary Freeze",
            "M2 Shared Contract Scaffolding",
            "M3 Harness Contract Design",
        ],
    ),
    (
        "docs/strategy/session-substrate/tasks.md",
        &[
            "Replace the current project-positioning drift guard with session-substrate checks",
            "Add shared session lifecycle types",
            "Add runner trait seams for session coordination",
        ],
    ),
    (
        "docs/strategy/session-substrate/context.yaml",
        &[
            "thesis:",
            "current_phase:",
            "core_abstractions:",
            "open_questions:",
        ],
    ),
    (
        "docs/adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md",
        &[
            "What Stays",
            "What Changes",
            "What Is Deferred",
            "Why This Is An Evolution, Not Random Thrash",
        ],
    ),
    (
        "docs/roadmap.md",
        &[
            "strategy/session-substrate/00-umbrella-epic.md",
            "strategy/session-substrate/07-roadmap.md",
            "Current Milestone",
        ],
    ),
    ("docs/adr/README.md", &["ADR `0020`", "session-substrate"]),
    (
        "docs/how-guild-works.md",
        &[
            "strategy/session-substrate/00-umbrella-epic.md",
            "This page still explains the current shipped skill-first runtime slice.",
        ],
    ),
    (
        "docs/command-language.md",
        &[
            "strategy/session-substrate/00-umbrella-epic.md",
            "The current command surface still uses the live internal family names",
        ],
    ),
    (
        "docs/testing.md",
        &[
            "strategy/session-substrate/00-umbrella-epic.md",
            "cargo run -q -p xtask -- project-positioning check",
        ],
    ),
];

const REQUIRED_LINKS: &[(&str, &[&str])] = &[
    (
        "README.md",
        &[
            "AGENTS.md",
            "docs/strategy/session-substrate/00-umbrella-epic.md",
            "docs/strategy/session-substrate/07-roadmap.md",
            "docs/strategy/session-substrate/tasks.md",
            "docs/adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md",
        ],
    ),
    (
        "docs/project-positioning.md",
        &[
            "strategy/session-substrate/00-umbrella-epic.md",
            "strategy/session-substrate/01-north-star.md",
            "strategy/session-substrate/07-roadmap.md",
            "strategy/session-substrate/tasks.md",
            "adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md",
        ],
    ),
    (
        "docs/roadmap.md",
        &[
            "strategy/session-substrate/00-umbrella-epic.md",
            "strategy/session-substrate/07-roadmap.md",
            "strategy/session-substrate/tasks.md",
        ],
    ),
    (
        "docs/contracts.md",
        &["strategy/session-substrate/00-umbrella-epic.md"],
    ),
    (
        "docs/architecture.md",
        &["strategy/session-substrate/00-umbrella-epic.md"],
    ),
    (
        "docs/how-guild-works.md",
        &["strategy/session-substrate/00-umbrella-epic.md"],
    ),
    (
        "docs/command-language.md",
        &["strategy/session-substrate/00-umbrella-epic.md"],
    ),
    (
        "docs/testing.md",
        &["strategy/session-substrate/00-umbrella-epic.md"],
    ),
];

pub fn check() -> Result<()> {
    let documents = load_documents()?;
    let base = repo_root();

    for (path, document) in &documents {
        validate_markdown_links(&base.join(path), path, document)?;
    }

    for (path, snippets) in REQUIRED_SNIPPETS {
        let document = document(&documents, path)?;
        let normalized = normalize_whitespace(document);
        for snippet in *snippets {
            if !normalized.contains(&normalize_whitespace(snippet)) {
                bail!("direction check: `{path}` is missing required snippet `{snippet}`");
            }
        }
    }

    for (path, links) in REQUIRED_LINKS {
        let document = document(&documents, path)?;
        for link in *links {
            ensure_link_exists(path, document, link)?;
        }
    }

    Ok(())
}

fn validate_markdown_links(
    document: &Path,
    document_path: &str,
    document_text: &str,
) -> Result<()> {
    for link in extract_markdown_links(document_text) {
        let Some(target) = resolve_local_markdown_link(document, &link) else {
            continue;
        };
        if !target.exists() {
            bail!(
                "direction check: markdown link `{link}` in `{document_path}` does not resolve to a real file",
            );
        }
    }

    Ok(())
}

fn load_documents() -> Result<BTreeMap<String, String>> {
    let root = repo_root();
    let mut documents = BTreeMap::new();

    for relative_path in REQUIRED_DOCS {
        let path = root.join(relative_path);
        if !path.is_file() {
            bail!(
                "required direction document `{relative_path}` is missing at {}",
                path.display()
            );
        }

        let contents = read_to_string(&path)
            .with_context(|| format!("failed to load direction document `{relative_path}`"))?;
        documents.insert((*relative_path).into(), contents);
    }

    Ok(documents)
}

fn document<'a>(documents: &'a BTreeMap<String, String>, relative_path: &str) -> Result<&'a str> {
    documents
        .get(relative_path)
        .map(String::as_str)
        .with_context(|| format!("direction check is missing loaded document `{relative_path}`"))
}

fn ensure_link_exists(document_path: &str, document_text: &str, link: &str) -> Result<()> {
    let base = repo_root();
    let document = base.join(document_path);
    let resolved = resolve_local_markdown_link(&document, link)
        .with_context(|| format!("failed to resolve `{link}` from `{document_path}`"))?;

    let link_present = extract_markdown_links(document_text)
        .into_iter()
        .filter_map(|candidate| resolve_local_markdown_link(&document, &candidate))
        .any(|candidate| candidate == resolved);

    if !link_present {
        bail!("direction check: link `{link}` is missing from `{document_path}`");
    }

    if !resolved.is_file() {
        bail!(
            "direction check: link `{link}` in `{document_path}` does not resolve to a real file",
        );
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

    let parent = document_path.parent()?;
    Some(parent.join(path_part))
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

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        extract_markdown_links, markdown_link_destination, resolve_local_markdown_link,
        validate_markdown_links,
    };

    #[test]
    fn extract_markdown_links_reads_inline_destinations() {
        let text = "See [umbrella](docs/strategy/session-substrate/00-umbrella-epic.md) and [roadmap](docs/roadmap.md).";
        let links = extract_markdown_links(text);
        assert_eq!(
            links,
            vec![
                "docs/strategy/session-substrate/00-umbrella-epic.md",
                "docs/roadmap.md"
            ]
        );
    }

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
    fn resolve_local_markdown_link_resolves_relative_targets() {
        let document = Path::new("/tmp/repo/docs/project-positioning.md");
        let resolved =
            resolve_local_markdown_link(document, "strategy/session-substrate/00-umbrella-epic.md")
                .unwrap();
        assert_eq!(
            resolved,
            Path::new("/tmp/repo/docs/strategy/session-substrate/00-umbrella-epic.md")
        );
    }

    #[test]
    fn resolve_local_markdown_link_ignores_external_targets() {
        let document = Path::new("/tmp/repo/README.md");
        assert!(resolve_local_markdown_link(document, "https://example.com").is_none());
        assert!(resolve_local_markdown_link(document, "#fragment").is_none());
        assert!(resolve_local_markdown_link(document, "mailto:test@example.com").is_none());
    }

    #[test]
    fn validate_markdown_links_rejects_broken_non_required_links() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("guild-project-positioning-{unique}"));
        let docs_dir = root.join("docs");
        fs::create_dir_all(&docs_dir).unwrap();

        let document_path = docs_dir.join("project-positioning.md");
        fs::write(&document_path, "See [broken](missing.md) for more.").unwrap();

        let result = validate_markdown_links(
            &document_path,
            document_path.strip_prefix(&root).unwrap().to_str().unwrap(),
            &fs::read_to_string(&document_path).unwrap(),
        );

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("does not resolve to a real file")
        );

        fs::remove_dir_all(&root).unwrap();
    }
}
