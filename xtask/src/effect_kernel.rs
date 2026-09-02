use std::{collections::BTreeSet, path::Path, process::Command};

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

const CRATES_IO_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependencyKind {
    Normal,
    Dev,
}

struct DependencySpec {
    name: &'static str,
    requirement: &'static str,
    kind: DependencyKind,
    features: &'static [&'static str],
}

const EXPECTED_DEPENDENCIES: &[DependencySpec] = &[
    DependencySpec {
        name: "hex",
        requirement: "=0.4.3",
        kind: DependencyKind::Normal,
        features: &[],
    },
    DependencySpec {
        name: "serde",
        requirement: "=1.0.228",
        kind: DependencyKind::Normal,
        features: &["derive"],
    },
    DependencySpec {
        name: "serde_jcs",
        requirement: "=0.1.0",
        kind: DependencyKind::Normal,
        features: &[],
    },
    DependencySpec {
        name: "serde_json",
        requirement: "=1.0.145",
        kind: DependencyKind::Normal,
        features: &[],
    },
    DependencySpec {
        name: "sha2",
        requirement: "=0.10.9",
        kind: DependencyKind::Normal,
        features: &[],
    },
    DependencySpec {
        name: "thiserror",
        requirement: "=2.0.17",
        kind: DependencyKind::Normal,
        features: &[],
    },
    DependencySpec {
        name: "proptest",
        requirement: "=1.8.0",
        kind: DependencyKind::Dev,
        features: &[],
    },
];

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

    validate_package(package)?;

    println!("effect-kernel dependency boundary: ok");
    Ok(())
}

fn validate_package(package: &Value) -> Result<()> {
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
    ensure!(
        dependencies.len() == EXPECTED_DEPENDENCIES.len(),
        "expected {} direct dependencies, found {}",
        EXPECTED_DEPENDENCIES.len(),
        dependencies.len()
    );
    let mut seen = BTreeSet::new();

    for dependency in dependencies {
        let name = dependency["name"]
            .as_str()
            .context("dependency metadata omitted name")?;
        if let Some(fragment) = FORBIDDEN_NAME_FRAGMENTS
            .iter()
            .find(|fragment| name.contains(**fragment))
        {
            bail!("forbidden dependency `{name}` matched `{fragment}`");
        }

        let kind = dependency_kind(dependency, name)?;
        match kind {
            DependencyKind::Normal => {
                ensure!(
                    ALLOWED_NORMAL.contains(&name),
                    "normal dependency `{name}` is outside the allowlist"
                );
            }
            DependencyKind::Dev => {
                ensure!(
                    ALLOWED_DEV.contains(&name),
                    "dev dependency `{name}` is outside the allowlist"
                );
            }
        }

        let spec = EXPECTED_DEPENDENCIES
            .iter()
            .find(|spec| spec.name == name)
            .context("dependency is outside the exact expected set")?;
        ensure!(seen.insert(name), "duplicate dependency `{name}`");
        validate_dependency(dependency, spec, kind)?;
    }

    for spec in EXPECTED_DEPENDENCIES {
        ensure!(
            seen.contains(spec.name),
            "missing dependency `{}`",
            spec.name
        );
    }
    Ok(())
}

fn dependency_kind(dependency: &Value, name: &str) -> Result<DependencyKind> {
    match dependency.get("kind") {
        Some(Value::Null) => Ok(DependencyKind::Normal),
        Some(Value::String(kind)) if kind == "dev" => Ok(DependencyKind::Dev),
        Some(Value::String(kind)) => bail!("dependency `{name}` has forbidden kind `{kind}`"),
        Some(_) => bail!("dependency `{name}` has invalid kind metadata"),
        None => bail!("dependency `{name}` metadata omitted kind"),
    }
}

fn validate_dependency(
    dependency: &Value,
    spec: &DependencySpec,
    kind: DependencyKind,
) -> Result<()> {
    let name = spec.name;
    ensure!(
        dependency["req"].as_str() == Some(spec.requirement),
        "dependency `{name}` must use exact requirement `{}`",
        spec.requirement
    );
    ensure!(
        kind == spec.kind,
        "dependency `{name}` has the wrong dependency kind"
    );
    ensure!(
        dependency["source"].as_str() == Some(CRATES_IO_SOURCE),
        "dependency `{name}` must come directly from crates.io"
    );
    ensure!(
        matches!(dependency.get("registry"), Some(Value::Null)),
        "dependency `{name}` must not select a custom registry"
    );
    ensure!(
        matches!(dependency.get("rename"), Some(Value::Null)),
        "dependency `{name}` must not be renamed"
    );
    ensure!(
        dependency.get("optional").and_then(Value::as_bool) == Some(false),
        "dependency `{name}` must not be optional"
    );
    ensure!(
        dependency
            .get("uses_default_features")
            .and_then(Value::as_bool)
            == Some(true),
        "dependency `{name}` must use default features"
    );
    ensure!(
        matches!(dependency.get("target"), Some(Value::Null)),
        "dependency `{name}` must not be target-specific"
    );
    let features = dependency
        .get("features")
        .and_then(Value::as_array)
        .context("dependency metadata omitted features")?
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .context("dependency features must be strings")?;
    ensure!(
        features == spec.features,
        "dependency `{name}` must use exactly the expected features"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::validate_package;

    const CRATES_IO_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

    fn dependency(name: &str, requirement: &str, kind: Option<&str>, features: &[&str]) -> Value {
        json!({
            "name": name,
            "source": CRATES_IO_SOURCE,
            "req": requirement,
            "kind": kind,
            "rename": null,
            "optional": false,
            "uses_default_features": true,
            "features": features,
            "target": null,
            "registry": null,
        })
    }

    fn valid_package() -> Value {
        json!({
            "name": "guild-effect-kernel",
            "rust_version": "1.94",
            "edition": "2024",
            "publish": [],
            "dependencies": [
                dependency("hex", "=0.4.3", None, &[]),
                dependency("serde", "=1.0.228", None, &["derive"]),
                dependency("serde_jcs", "=0.1.0", None, &[]),
                dependency("serde_json", "=1.0.145", None, &[]),
                dependency("sha2", "=0.10.9", None, &[]),
                dependency("thiserror", "=2.0.17", None, &[]),
                dependency("proptest", "=1.8.0", Some("dev"), &[]),
            ],
        })
    }

    fn dependency_mut<'a>(package: &'a mut Value, name: &str) -> &'a mut Value {
        package["dependencies"]
            .as_array_mut()
            .expect("dependencies are an array")
            .iter_mut()
            .find(|dependency| dependency["name"] == name)
            .expect("fixture contains dependency")
    }

    #[test]
    fn valid_dependency_metadata_is_accepted() {
        validate_package(&valid_package()).expect("valid package metadata is accepted");
    }

    #[test]
    fn serde_without_derive_is_rejected() {
        let mut package = valid_package();
        dependency_mut(&mut package, "serde")["features"] = json!([]);

        assert!(validate_package(&package).is_err());
    }

    #[test]
    fn allowed_names_with_non_crates_io_sources_are_rejected() {
        for (case, source) in [
            ("path", Value::Null),
            ("git", json!("git+https://example.com/serde?rev=deadbeef")),
            (
                "alternate registry",
                json!("registry+https://example.com/index"),
            ),
        ] {
            let mut package = valid_package();
            dependency_mut(&mut package, "serde")["source"] = source;

            assert!(
                validate_package(&package).is_err(),
                "{case} dependency source must be rejected"
            );
        }
    }

    #[test]
    fn unexpected_dependency_attributes_are_rejected() {
        for (case, field, value) in [
            ("features", "features", json!(["derive", "rc"])),
            ("default features", "uses_default_features", json!(false)),
            ("optional", "optional", json!(true)),
            ("target", "target", json!("cfg(unix)")),
            ("rename", "rename", json!("serde_alias")),
            ("registry", "registry", json!("custom")),
        ] {
            let mut package = valid_package();
            dependency_mut(&mut package, "serde")[field] = value;

            assert!(
                validate_package(&package).is_err(),
                "unexpected {case} must be rejected"
            );
        }
    }
}
