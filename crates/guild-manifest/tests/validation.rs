use guild_manifest::{
    BehaviorSpec, BuildProfile, InstalledDependencySpec, InterfaceSpec, ManifestValidationError,
    ModePolicy, PublisherRef, RuntimeSpec, SkillManifest, SourceBuildKind, SourceBuildSpec,
    SourceDependencySpec, SourcePackageSpec, SourceSkillManifest, TestSpec, TrustTier, Visibility,
};
use guild_types::{
    AbiVersion, CapabilityAccess, CapabilityConstraints, CapabilityId, CapabilityRequirement,
    EmitEvidenceConstraints, ExecutionMode, FreshnessClass, Mutability, ReadResourceConstraints,
    RequestedSkillRef, ResolvedSkillRef, ResourceKind, RuntimeKind, SkillCategory, SkillKey,
    SkillVersion, VersionRequirement,
};

fn sample_source_manifest() -> SourceSkillManifest {
    SourceSkillManifest {
        api_version: AbiVersion::GuildSkillV1,
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-inspect".into(),
        },
        version: SkillVersion::parse("0.1.0").unwrap(),
        display_name: "Hello Inspect".into(),
        description: "A tiny inspect-only example skill.".into(),
        runtime: RuntimeSpec {
            kind: RuntimeKind::WasmComponent,
            entrypoint: "guild-skill".into(),
            abi: AbiVersion::GuildSkillV1,
        },
        interface: InterfaceSpec {
            input_schema_uri: "./input.schema.json".into(),
            output_schema_uri: "./output.schema.json".into(),
            examples_uri: Some("./examples.json".into()),
        },
        behavior: BehaviorSpec {
            category: SkillCategory::Explain,
            mutability: Mutability::ReadOnly,
            idempotent: true,
            open_world: false,
            freshness: FreshnessClass::Deterministic,
            modes: ModePolicy {
                supported: vec![ExecutionMode::Inspect],
                apply_requires_approval: false,
                apply_requires_idempotency_key: false,
            },
        },
        capabilities: vec![CapabilityRequirement {
            id: CapabilityId::ReadResource,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                uri_prefixes: Some(vec!["guild://executions/".into()]),
                resource_kinds: Some(vec![ResourceKind::Execution]),
            }),
            required: true,
        }],
        dependencies: vec![SourceDependencySpec {
            alias: "dependency".into(),
            skill: RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "dependency".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
        }],
        publisher: PublisherRef {
            id: "local.example".into(),
            display_name: "Local Example".into(),
            homepage: None,
        },
        package: SourcePackageSpec {
            visibility: Visibility::Private,
            trust_tier: TrustTier::Local,
            sbom_uri: None,
            signature_uri: None,
        },
        build: SourceBuildSpec {
            kind: SourceBuildKind::CargoWasmComponent,
            cargo_manifest_path: "./skill-rust/Cargo.toml".into(),
            target: "wasm32-wasip2".into(),
            profile: BuildProfile::Release,
        },
        tests: vec![TestSpec {
            name: "greets-in-inspect-mode".into(),
            fixtures_uri: "./tests/inspect-input.json".into(),
            expected_output_uri: "./tests/expected-output.json".into(),
        }],
    }
}

fn has_error(errors: &[ManifestValidationError], path: &str) -> bool {
    errors.iter().any(|error| error.path == path)
}

#[test]
fn source_manifest_roundtrips_typed_versions_and_mode_policy() {
    let manifest = sample_source_manifest();
    let encoded = serde_json::to_string_pretty(&manifest).unwrap();
    let decoded: SourceSkillManifest = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.version.to_string(), "0.1.0");
    assert_eq!(
        decoded.dependencies[0].skill.version_req.to_string(),
        "^0.1"
    );
    assert_eq!(
        decoded.behavior.modes.supported,
        vec![ExecutionMode::Inspect]
    );
    assert_eq!(decoded, manifest);
}

#[test]
fn source_manifest_requires_unique_dependency_aliases() {
    let mut manifest = sample_source_manifest();
    manifest.dependencies.push(manifest.dependencies[0].clone());

    let errors = manifest.validate().unwrap_err();
    assert!(has_error(&errors, "dependencies[1].alias"));
}

#[test]
fn source_manifest_rejects_illegal_apply_mode_flags() {
    let mut manifest = sample_source_manifest();
    manifest.behavior.modes.apply_requires_approval = true;
    manifest.behavior.modes.apply_requires_idempotency_key = true;

    let errors = manifest.validate().unwrap_err();
    assert!(has_error(&errors, "behavior.modes.apply_requires_approval"));
    assert!(has_error(
        &errors,
        "behavior.modes.apply_requires_idempotency_key"
    ));
}

#[test]
fn source_manifest_converts_to_installed_manifest() {
    let source = sample_source_manifest();
    let installed: SkillManifest = source.clone().into_installed(
        "./component.wasm",
        "sha256:abc123",
        vec![InstalledDependencySpec {
            alias: "dependency".into(),
            skill: ResolvedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "dependency".into(),
                },
                version: SkillVersion::parse("0.1.5").unwrap(),
                digest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .into(),
            },
        }],
    );

    assert_eq!(installed.package.artifact_uri, "./component.wasm");
    assert_eq!(installed.package.artifact_digest, "sha256:abc123");
    assert_eq!(installed.version, source.version);
    assert_eq!(installed.dependencies[0].alias, "dependency");
    assert_eq!(installed.dependencies[0].skill.version.to_string(), "0.1.5");
}

#[test]
fn example_fixture_source_manifest_is_valid_and_inspect_only() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/skills/hello-inspect/manifest.json");
    let manifest: SourceSkillManifest =
        serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();

    manifest.validate().unwrap();
    assert_eq!(manifest.runtime.kind, RuntimeKind::WasmComponent);
    assert_eq!(manifest.version.to_string(), "0.1.0");
    assert_eq!(
        manifest.behavior.modes.supported,
        vec![ExecutionMode::Inspect]
    );
    assert_eq!(manifest.build.kind, SourceBuildKind::CargoWasmComponent);
    assert_eq!(
        manifest.build.cargo_manifest_path,
        "./skill-rust/Cargo.toml"
    );
}

#[test]
fn composite_fixture_source_manifest_declares_alias_scoped_dependency() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/skills/hello-composite/manifest.json");
    let manifest: SourceSkillManifest =
        serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();

    manifest.validate().unwrap();
    assert_eq!(manifest.dependencies.len(), 1);
    assert_eq!(manifest.dependencies[0].alias, "hello");
    assert_eq!(manifest.dependencies[0].skill.key.name, "hello-inspect");
    assert_eq!(
        manifest.capabilities[0].constraints,
        CapabilityConstraints::InvokeDependency(guild_types::InvokeDependencyConstraints {
            aliases: Some(vec!["hello".into()]),
        })
    );
}

#[test]
fn explain_fixture_source_manifest_declares_scoped_resource_reads() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/skills/explain-execution/manifest.json");
    let manifest: SourceSkillManifest =
        serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();

    manifest.validate().unwrap();
    assert_eq!(manifest.capabilities.len(), 2);
    assert_eq!(
        manifest.capabilities[0].id,
        guild_types::CapabilityId::ReadResource
    );
    assert_eq!(
        manifest.capabilities[0].constraints,
        CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://executions/".into()]),
            resource_kinds: Some(vec![ResourceKind::Execution]),
        })
    );
    assert_eq!(
        manifest.capabilities[1].constraints,
        CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://objects/sha256/".into()]),
            resource_kinds: Some(vec![ResourceKind::Object]),
        })
    );
}

#[test]
fn capability_validation_rejects_wrong_family_and_empty_scopes() {
    let mut manifest = sample_source_manifest();
    manifest.capabilities = vec![
        CapabilityRequirement {
            id: CapabilityId::LogWrite,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                uri_prefixes: Some(vec!["guild://executions/".into()]),
                resource_kinds: Some(vec![ResourceKind::Execution]),
            }),
            required: false,
        },
        CapabilityRequirement {
            id: CapabilityId::EmitEvidence,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                max_bytes: Some(0),
                audiences: Some(Vec::new()),
                redactions: Some(Vec::new()),
            }),
            required: true,
        },
    ];

    let errors = manifest.validate().unwrap_err();
    assert!(has_error(&errors, "capabilities[0].constraints"));
    assert!(has_error(&errors, "capabilities[1].constraints"));
}
