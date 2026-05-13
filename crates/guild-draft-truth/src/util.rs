use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Map, Value};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

pub fn draft_v1_dir() -> PathBuf {
    repo_root().join("docs/schemas/draft-v1")
}

pub fn benchmarking_dir() -> PathBuf {
    repo_root().join("docs/benchmarking")
}

pub fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

pub fn read_json(path: &Path) -> Result<Value> {
    let text = read_to_string(path)?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse JSON from {}", path.display()))
}

pub fn write_json_pretty(path: &Path, value: &Value) -> Result<()> {
    let rendered = format!("{}\n", serde_json::to_string_pretty(value)?);
    fs::write(path, rendered).with_context(|| format!("failed to write {}", path.display()))
}

pub fn write_string(path: &Path, value: &str) -> Result<()> {
    fs::write(path, value).with_context(|| format!("failed to write {}", path.display()))
}

pub fn ensure_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create parent directory for {}", path.display()))
}

pub fn json_object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .with_context(|| format!("{context} should be a JSON object"))
}

pub fn json_array<'a>(value: &'a Value, context: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .with_context(|| format!("{context} should be a JSON array"))
}

pub fn get_required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a Value> {
    object
        .get(key)
        .with_context(|| format!("{context} is missing required key `{key}`"))
}

pub fn get_required_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a str> {
    get_required(object, key, context)?
        .as_str()
        .with_context(|| format!("{context}.{key} should be a string"))
}

pub fn get_optional_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Option<&'a str>> {
    match object.get(key) {
        Some(value) => {
            Ok(Some(value.as_str().with_context(|| {
                format!("{context}.{key} should be a string")
            })?))
        }
        None => Ok(None),
    }
}

pub fn get_optional_bool(
    object: &Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Option<bool>> {
    match object.get(key) {
        Some(value) => {
            Ok(Some(value.as_bool().with_context(|| {
                format!("{context}.{key} should be a boolean")
            })?))
        }
        None => Ok(None),
    }
}

pub fn get_optional_u64(
    object: &Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Option<u64>> {
    match object.get(key) {
        Some(value) => Ok(Some(value.as_u64().with_context(|| {
            format!("{context}.{key} should be an unsigned integer")
        })?)),
        None => Ok(None),
    }
}

pub fn stable_unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        if !output.contains(&value) {
            output.push(value);
        }
    }
    output
}

pub fn canonical_json(value: &Value) -> Result<String> {
    serde_json::to_string(value).context("failed to render canonical JSON")
}

pub fn json_digest(value: &Value) -> Result<Value> {
    use sha2::{Digest, Sha256};

    let rendered = canonical_json(value)?;
    let digest = Sha256::digest(rendered.as_bytes());
    Ok(serde_json::json!({
        "algorithm": "sha256",
        "value": hex::encode(digest),
    }))
}

pub fn run_cargo_json(args: &[&str]) -> Result<Value> {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("failed to run cargo {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "cargo {} failed:\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            stdout.trim(),
            stderr.trim()
        );
    }
    serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "failed to decode JSON from `cargo {}` output",
            args.join(" ")
        )
    })
}

pub fn measure_operation<F, T>(
    mut factory: F,
    warmup_runs: usize,
    measured_runs: usize,
    cache_present: bool,
    cache_notes: &str,
) -> Result<(T, Vec<T>, Value)>
where
    F: FnMut() -> Result<T>,
    T: Clone,
{
    let (cold_value, cold_ms) = timed_call(&mut factory)?;
    for _ in 0..warmup_runs {
        let _ = factory()?;
    }

    let mut measured_values = Vec::with_capacity(measured_runs);
    let mut samples = Vec::with_capacity(measured_runs);
    for _ in 0..measured_runs {
        let (value, elapsed_ms) = timed_call(&mut factory)?;
        measured_values.push(value);
        samples.push(elapsed_ms);
    }

    let timing = timing_summary(cold_ms, &samples, warmup_runs, cache_present, cache_notes)?;
    Ok((cold_value, measured_values, timing))
}

pub fn timed_call<F, T>(factory: &mut F) -> Result<(T, f64)>
where
    F: FnMut() -> Result<T>,
{
    let started = std::time::Instant::now();
    let value = factory()?;
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok((value, elapsed_ms))
}

pub fn timing_summary(
    cold_first_run_ms: f64,
    samples: &[f64],
    warmup_runs: usize,
    cache_present: bool,
    cache_notes: &str,
) -> Result<Value> {
    let measured_runs = samples.len();
    let mean_ms = if measured_runs == 0 {
        0.0
    } else {
        samples.iter().sum::<f64>() / measured_runs as f64
    };
    let p50_ms = percentile(samples, 0.50);
    let p95_ms = percentile(samples, 0.95);
    let max_ms = samples.iter().copied().fold(0.0, f64::max);
    Ok(serde_json::json!({
        "cold_first_run_ms": cold_first_run_ms,
        "warmup_runs": warmup_runs,
        "measured_runs": measured_runs,
        "samples_ms": samples,
        "mean_ms": mean_ms,
        "p50_ms": p50_ms,
        "p95_ms": p95_ms,
        "max_ms": max_ms,
        "cache_present": cache_present,
        "cache_notes": cache_notes,
    }))
}

pub fn percentile(samples: &[f64], percentile_value: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut ordered = samples.to_vec();
    ordered.sort_by(f64::total_cmp);
    let rank = ((ordered.len() - 1) as f64 * percentile_value).round() as usize;
    ordered[rank]
}

pub fn count_rate(values: &[String], target: &str) -> Value {
    let count = values
        .iter()
        .filter(|value| value.as_str() == target)
        .count();
    let rate = if values.is_empty() {
        0.0
    } else {
        count as f64 / values.len() as f64
    };
    serde_json::json!({
        "count": count,
        "rate": rate,
    })
}

pub fn to_pretty_string<T: Serialize>(value: &T) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}
