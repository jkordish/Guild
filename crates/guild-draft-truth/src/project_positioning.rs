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
        for link in *links {
            ensure_link_exists(path, link)?;
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

fn ensure_link_exists(document_path: &str, link: &str) -> Result<()> {
    let base = repo_root();
    let document = base.join(document_path);
    let resolved = resolve_relative_link(&document, link)
        .with_context(|| format!("failed to resolve `{link}` from `{document_path}`"))?;

    if !resolved.is_file() {
        bail!(
            "direction check: link `{link}` in `{document_path}` does not resolve to a real file",
        );
    }

    Ok(())
}

fn resolve_relative_link(document_path: &Path, link: &str) -> Result<PathBuf> {
    let target = link
        .split('#')
        .next()
        .filter(|segment| !segment.is_empty())
        .context("link target was empty")?;
    let parent = document_path
        .parent()
        .with_context(|| format!("{} has no parent directory", document_path.display()))?;
    Ok(parent.join(target))
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
