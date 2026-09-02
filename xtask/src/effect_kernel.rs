use std::{collections::BTreeMap, path::Path, process::Command};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

const ALLOWED_NORMAL: &[&str] = &[
    "hex",
    "serde",
    "serde_jcs",
    "serde_json",
    "sha2",
    "thiserror",
];
const ALLOWED_DEV: &[&str] = &["proptest"];
const FORBIDDEN_NAME_FRAGMENTS: &[&str] = &[
    "guild-",
    "tokio",
    "reqwest",
    "hyper",
    "rusqlite",
    "sqlx",
    "wasmtime",
    "wit-bindgen",
    "uuid",
    "rand",
    "getrandom",
];

const EXPECTED_NORMAL_REQUIREMENTS: &[(&str, &str)] = &[
    ("hex", "=0.4.3"),
    ("serde", "=1.0.228"),
    ("serde_jcs", "=0.1.0"),
    ("serde_json", "=1.0.145"),
    ("sha2", "=0.10.9"),
    ("thiserror", "=2.0.17"),
];
const EXPECTED_DEV_REQUIREMENTS: &[(&str, &str)] = &[("proptest", "=1.8.0")];

pub fn run(mut args: impl Iterator<Item = String>) -> Result<()> {
    let Some(command) = args.next() else {
        bail!("usage: cargo run -p xtask -- effect-kernel check-dependencies");
    };
    ensure!(
        args.next().is_none(),
        "effect-kernel accepts exactly one argument"
    );
    ensure!(
        command == "check-dependencies",
        "unknown effect-kernel command `{command}`"
    );

    check_dependencies()
}

fn check_dependencies() -> Result<()> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask manifest directory has no workspace parent")?;
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .context("failed to run cargo metadata")?;
    ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );

    let metadata: Value =
        serde_json::from_slice(&output.stdout).context("cargo metadata returned invalid JSON")?;
    let packages = metadata["packages"]
        .as_array()
        .context("cargo metadata omitted packages")?;
    let package = packages
        .iter()
        .find(|package| package["name"] == "guild-effect-kernel")
        .context("cargo metadata omitted guild-effect-kernel")?;

    ensure!(
        package["rust_version"].as_str() == Some("1.94"),
        "guild-effect-kernel must declare rust-version 1.94"
    );
    ensure!(
        package["edition"].as_str() == Some("2024"),
        "guild-effect-kernel must use edition 2024"
    );
    let publish = package["publish"]
        .as_array()
        .context("guild-effect-kernel must declare publish = false")?;
    ensure!(
        publish.is_empty(),
        "guild-effect-kernel must declare publish = false"
    );

    let dependencies = package["dependencies"]
        .as_array()
        .context("guild-effect-kernel metadata omitted dependencies")?;
    let mut normal = BTreeMap::new();
    let mut dev = BTreeMap::new();

    for dependency in dependencies {
        let name = dependency["name"]
            .as_str()
            .context("dependency metadata omitted name")?;
        let requirement = dependency["req"]
            .as_str()
            .context("dependency metadata omitted requirement")?;

        if let Some(fragment) = FORBIDDEN_NAME_FRAGMENTS
            .iter()
            .find(|fragment| name.contains(**fragment))
        {
            bail!("forbidden dependency `{name}` matched `{fragment}`");
        }

        match dependency["kind"].as_str() {
            None => {
                ensure!(
                    ALLOWED_NORMAL.contains(&name),
                    "normal dependency `{name}` is outside the allowlist"
                );
                ensure!(
                    normal.insert(name, requirement).is_none(),
                    "duplicate normal dependency `{name}`"
                );
            }
            Some("dev") => {
                ensure!(
                    ALLOWED_DEV.contains(&name),
                    "dev dependency `{name}` is outside the allowlist"
                );
                ensure!(
                    dev.insert(name, requirement).is_none(),
                    "duplicate dev dependency `{name}`"
                );
            }
            Some(kind) => bail!("dependency `{name}` has forbidden kind `{kind}`"),
        }
    }

    check_requirements("normal", &normal, EXPECTED_NORMAL_REQUIREMENTS)?;
    check_requirements("dev", &dev, EXPECTED_DEV_REQUIREMENTS)?;

    println!("effect-kernel dependency boundary: ok");
    Ok(())
}

fn check_requirements(
    kind: &str,
    actual: &BTreeMap<&str, &str>,
    expected: &[(&str, &str)],
) -> Result<()> {
    ensure!(
        actual.len() == expected.len(),
        "expected {} {kind} dependencies, found {}",
        expected.len(),
        actual.len()
    );
    for &(name, requirement) in expected {
        ensure!(
            actual.get(name).copied() == Some(requirement),
            "{kind} dependency `{name}` must use exact requirement `{requirement}`"
        );
    }
    Ok(())
}
