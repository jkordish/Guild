use std::collections::BTreeMap;
use std::fmt::Write as _;

use guild_manifest::SkillManifest;
use guild_registry::{InstalledSkill, RegistryError, SignatureScheme, TrustedPublisherRecord};
use guild_types::{
    AbiVersion, AuthorityObservation, AuthorityObservationStatus, CapabilityAccess,
    CapabilityConstraints, CapabilityId, CapabilityRequirement, ChildExecutionRecord,
    EvidenceAudience, EvidenceBlobRecord, EvidenceRecord, ExecutionPhase, ExecutionRecord,
    ExecutionStatus, GUILD_EXECUTION_QUERY_URI_PREFIX, GUILD_EXECUTION_URI_PREFIX,
    GUILD_OBJECT_BLOB_URI_PREFIX, GUILD_OBJECT_RECORD_METADATA_URI_SUFFIX,
    GUILD_OBJECT_RECORD_URI_PREFIX, GrantedCapability, GuildResourceUri, HttpMethod, HttpScheme,
    LocalTrustTier, PRESENTATION_STATUS_LINKED, PRESENTATION_STATUS_PROOF_BACKED,
    PRESENTATION_STATUS_REFUSED, PRESENTATION_STATUS_UNLINKED, PRESENTATION_STATUS_UPPER_BOUND,
    RedactionClass, ResolvedSkillRef, ResourceKind, RuntimeKind, SUPPORT_STATUS_BOUNDED,
    SUPPORT_STATUS_NOT_PROVEN, Severity, SkillCategory, TerminationDetail, execution_status_label,
};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy)]
pub struct PresentationOptions {
    pub verbosity: u8,
    pub debug: bool,
    pub color: ColorMode,
    pub stdout_is_terminal: bool,
    pub stderr_is_terminal: bool,
}

impl PresentationOptions {
    #[must_use]
    pub fn styler(self, stream: StreamKind) -> Styler {
        let enabled = match self.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => match stream {
                StreamKind::Stdout => self.stdout_is_terminal,
                StreamKind::Stderr => self.stderr_is_terminal,
            },
        };
        Styler { enabled }
    }

    #[must_use]
    pub fn verbose(self) -> bool {
        self.verbosity >= 1 || self.debug
    }

    #[must_use]
    pub fn very_verbose(self) -> bool {
        self.verbosity >= 2 || self.debug
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SupportBucket {
    pub status: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SupportSummary {
    pub overall: String,
    pub buckets: Vec<SupportBucket>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WhySummary {
    pub plan: String,
    pub proof: String,
    pub token: String,
    pub witness: String,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhyLineageNode {
    pub depth: usize,
    pub alias_from_parent: Option<String>,
    pub record: ExecutionRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhyLineageWarning {
    pub relation: String,
    pub code: String,
    pub message: String,
    pub execution_uri: Option<String>,
    pub depth: usize,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhyLineage {
    pub ancestry: Vec<ExecutionRecord>,
    pub descendants: Vec<WhyLineageNode>,
    pub warnings: Vec<WhyLineageWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthorityDiffChange {
    Same,
    Changed,
    RequestedOnly,
    GrantedOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthorityDiffGroup {
    id: CapabilityId,
    access: CapabilityAccess,
    requested: Vec<GrantedCapability>,
    granted: Vec<GrantedCapability>,
    change: AuthorityDiffChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Success,
    Warning,
    Danger,
    Ref,
    Type,
    Dim,
}

#[derive(Debug, Clone, Copy)]
pub struct Styler {
    enabled: bool,
}

impl Styler {
    #[must_use]
    fn paint(self, tone: Tone, text: impl AsRef<str>) -> String {
        let text = text.as_ref();
        if !self.enabled {
            return text.to_owned();
        }

        let code = match tone {
            Tone::Success => "32",
            Tone::Warning => "33",
            Tone::Danger => "31",
            Tone::Ref => "36",
            Tone::Type => "35",
            Tone::Dim => "2",
        };
        format!("\u{1b}[{code}m{text}\u{1b}[0m")
    }
}

#[must_use]
pub fn color_mode(no_color: bool, requested: Option<&str>) -> ColorMode {
    if no_color {
        return ColorMode::Never;
    }

    match requested {
        Some("always") => ColorMode::Always,
        Some("never") => ColorMode::Never,
        _ => ColorMode::Auto,
    }
}

#[must_use]
pub fn short_skill_ref(installed: &InstalledSkill) -> String {
    short_resolved_skill_ref(&installed.resolved_ref)
}

#[must_use]
pub fn short_execution_ref(record: &ExecutionRecord) -> String {
    short_prefixed_id("exec", &record.receipt.execution_id)
}

#[must_use]
pub fn short_child_execution_ref(record: &ChildExecutionRecord) -> String {
    short_prefixed_id("exec", &record.execution_id)
}

#[must_use]
pub fn short_evidence_ref(record: &EvidenceRecord) -> String {
    record.uri.rsplit('/').next().map_or_else(
        || record.uri.clone(),
        |id| short_prefixed_id("evidence", id),
    )
}

#[must_use]
pub fn short_object_ref(record: &EvidenceBlobRecord) -> String {
    format!("obj:{}", short_hash(&record.sha256))
}

#[must_use]
pub fn runtime_label(manifest: &SkillManifest) -> String {
    format!(
        "{} / {}",
        runtime_kind_label(&manifest.runtime.kind),
        manifest.runtime.entrypoint
    )
}

#[must_use]
pub fn resolved_skill_ref(skill: &ResolvedSkillRef) -> String {
    format!(
        "skill://{}/{}@{}",
        skill.key.namespace, skill.key.name, skill.version
    )
}

#[must_use]
pub fn short_resolved_skill_ref(skill: &ResolvedSkillRef) -> String {
    format!(
        "{}/{}@{}",
        skill.key.namespace, skill.key.name, skill.version
    )
}

#[must_use]
pub fn support_summary_for_skill(installed: &InstalledSkill) -> SupportSummary {
    support_summary_for_capabilities(installed.manifest.capabilities.iter().map(|cap| &cap.id))
}

#[must_use]
pub fn support_summary_for_execution(record: &ExecutionRecord) -> SupportSummary {
    let observed = observed_capability_ids(record);
    if observed.is_empty() {
        support_summary_for_capabilities(
            record.granted_capabilities.grants.iter().map(|cap| &cap.id),
        )
    } else {
        support_summary_for_capabilities(observed)
    }
}

#[must_use]
pub fn why_summary(record: &ExecutionRecord) -> WhySummary {
    let proof = overall_support_word(&support_summary_for_execution(record));
    let plan = if matches!(record.status, ExecutionStatus::Rejected) {
        PRESENTATION_STATUS_REFUSED
    } else {
        PRESENTATION_STATUS_UPPER_BOUND
    };
    let token = if matches!(record.status, ExecutionStatus::Rejected) {
        PRESENTATION_STATUS_REFUSED
    } else if proof == PRESENTATION_STATUS_PROOF_BACKED {
        PRESENTATION_STATUS_LINKED
    } else {
        PRESENTATION_STATUS_UPPER_BOUND
    };
    let witness = if proof == PRESENTATION_STATUS_PROOF_BACKED {
        PRESENTATION_STATUS_LINKED
    } else {
        PRESENTATION_STATUS_UNLINKED
    };

    WhySummary {
        plan: plan.into(),
        proof: proof.into(),
        token: token.into(),
        witness: witness.into(),
        reason_codes: reason_codes(record),
    }
}

#[must_use]
pub fn render_skill_show(
    installed: &InstalledSkill,
    requested: &str,
    resolution_lines: &[String],
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    let support = support_summary_for_skill(installed);
    let verification = installed.trust.verification_state.to_string();
    let trust = installed.trust.trust_tier.to_string();
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}  {}",
        styler.paint(Tone::Ref, short_skill_ref(installed)),
        installed.manifest.display_name
    );
    let _ = writeln!(
        output,
        "status: {}",
        render_trust_status_pair(Some(styler), &verification, &trust)
    );
    let _ = writeln!(
        output,
        "support: {}",
        render_support_summary(&support, styler)
    );
    let _ = writeln!(
        output,
        "runtime: {}",
        styler.paint(Tone::Type, runtime_label(&installed.manifest))
    );
    let _ = writeln!(
        output,
        "caps: {}",
        capability_summary(&installed.manifest.capabilities)
    );
    if options.verbose() {
        let _ = writeln!(output, "requested: {}", styler.paint(Tone::Ref, requested));
        let _ = writeln!(
            output,
            "resolved: {}",
            styler.paint(Tone::Ref, resolved_skill_ref(&installed.resolved_ref))
        );
        let _ = writeln!(
            output,
            "digest: {}",
            styler.paint(Tone::Ref, installed.resolved_ref.digest.as_str())
        );
        let _ = writeln!(
            output,
            "category: {}",
            skill_category_label(&installed.manifest.behavior.category)
        );
        let _ = writeln!(
            output,
            "installed path: {}",
            styler.paint(Tone::Dim, installed.root_dir.display().to_string())
        );
    }
    if options.very_verbose() {
        if !resolution_lines.is_empty() {
            let _ = writeln!(output, "resolution:");
            for line in resolution_lines {
                let _ = writeln!(output, "  {line}");
            }
        }
        let _ = writeln!(output, "description: {}", installed.manifest.description);
        let _ = writeln!(
            output,
            "abi: {}",
            abi_version_label(&installed.manifest.runtime.guest_abi_version)
        );
        let _ = writeln!(
            output,
            "publisher: {}",
            styler.paint(Tone::Ref, installed.manifest.publisher.id.as_str())
        );
    }
    output
}

#[must_use]
pub fn render_skill_show_next_steps(installed: &InstalledSkill) -> String {
    let resolved = resolved_skill_ref(&installed.resolved_ref);
    format!("Next: guild verify {resolved}")
}

#[must_use]
pub fn render_skill_verify(
    installed: &InstalledSkill,
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    let verification = installed.trust.verification_state.to_string();
    let trust = installed.trust.trust_tier.to_string();
    let mut output = String::new();
    let publisher = installed
        .verification
        .as_ref()
        .map_or("local-source", |record| record.publisher.id.as_str());
    let _ = writeln!(
        output,
        "{}",
        styler.paint(Tone::Ref, short_skill_ref(installed))
    );
    let _ = writeln!(
        output,
        "publisher: {}",
        render_publisher_label(Some(styler), publisher)
    );
    let _ = writeln!(
        output,
        "status: {}",
        render_trust_status_pair(Some(styler), &verification, &trust)
    );
    if let Some(verification) = &installed.verification
        && options.verbose()
    {
        let _ = writeln!(
            output,
            "scheme: {}",
            signature_scheme_label(&verification.scheme)
        );
        let _ = writeln!(
            output,
            "bundle digest: {}",
            styler.paint(
                Tone::Ref,
                format!("sha256:{}", short_hash(&verification.bundle_sha256)),
            )
        );
    }
    output
}

#[must_use]
pub fn render_skill_verify_next_step(installed: &InstalledSkill) -> String {
    format!(
        "Next: guild show -v {}",
        resolved_skill_ref(&installed.resolved_ref)
    )
}

#[must_use]
pub fn render_transport_import_summary(
    format: &str,
    source: &str,
    installed_count: usize,
) -> String {
    let mut output = String::new();
    let noun = if installed_count == 1 {
        "skill"
    } else {
        "skills"
    };
    let _ = writeln!(output, "imported installed state");
    let _ = writeln!(output, "transport: {format}");
    let _ = writeln!(output, "source: {source}");
    let _ = writeln!(output, "installed: {installed_count} {noun}");
    output
}

#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn render_transport_import_preview_summary(
    format: &str,
    source: &str,
    decision: &str,
    root_skill: &str,
    includes_dependency_closure: bool,
    skill_count: usize,
    publisher_id: &str,
    signature_scheme: &SignatureScheme,
    bundle_sha256: &str,
    verified: bool,
    trust_tier: Option<&LocalTrustTier>,
    verification_error: Option<&RegistryError>,
    refusal: Option<&RegistryError>,
) -> String {
    let mut output = String::new();
    let contents = if includes_dependency_closure {
        "root skill plus dependency closure"
    } else {
        "root skill only"
    };
    let noun = if skill_count == 1 { "skill" } else { "skills" };
    let verification = if verified { "verified" } else { "refused" };
    let trust = trust_tier.map_or_else(|| "untrusted".to_owned(), std::string::ToString::to_string);

    let _ = writeln!(output, "previewed installed state");
    let _ = writeln!(output, "transport: {format}");
    let _ = writeln!(output, "source: {source}");
    let _ = writeln!(output, "decision: {decision}");
    let _ = writeln!(output, "skill: {root_skill}");
    let _ = writeln!(output, "publisher: {publisher_id}");
    let _ = writeln!(output, "status: {verification} / {trust}");
    let _ = writeln!(
        output,
        "scheme: {}",
        signature_scheme_label(signature_scheme)
    );
    let _ = writeln!(output, "bundle digest: {bundle_sha256}");
    let _ = writeln!(output, "contents: {contents}");
    let _ = writeln!(output, "skills: {skill_count} {noun}");
    if let Some(error) = refusal.or(verification_error) {
        let _ = writeln!(output, "reason: {}: {}", error.code, error.message);
    }
    output
}

#[must_use]
pub fn render_transport_export_summary(
    format: &str,
    root_skill: &str,
    publisher_id: &str,
    includes_dependency_closure: bool,
    output_root: &str,
) -> String {
    let mut output = String::new();
    let contents = if includes_dependency_closure {
        "root skill plus dependency closure"
    } else {
        "root skill only"
    };
    let _ = writeln!(output, "exported installed state");
    let _ = writeln!(output, "transport: {format}");
    let _ = writeln!(output, "skill: {root_skill}");
    let _ = writeln!(output, "publisher: {publisher_id}");
    let _ = writeln!(output, "contents: {contents}");
    let _ = writeln!(output, "output: {output_root}");
    output
}

#[must_use]
pub fn render_transport_export_next_step(format: &str, output_root: &str) -> String {
    match format {
        "bundle" => format!("Next: guild import bundle {output_root}"),
        "oci-layout" => format!("Next: guild import oci-layout {output_root}"),
        _ => "Next: guild help preview".to_owned(),
    }
}

#[must_use]
pub fn render_transport_push_summary(
    root_skill: &str,
    publisher_id: &str,
    includes_dependency_closure: bool,
    reference: &str,
    manifest_digest: &str,
) -> String {
    let mut output = String::new();
    let contents = if includes_dependency_closure {
        "root skill plus dependency closure"
    } else {
        "root skill only"
    };
    let _ = writeln!(output, "published installed state");
    let _ = writeln!(output, "transport: oci-registry");
    let _ = writeln!(output, "skill: {root_skill}");
    let _ = writeln!(output, "publisher: {publisher_id}");
    let _ = writeln!(output, "contents: {contents}");
    let _ = writeln!(output, "reference: {reference}");
    let _ = writeln!(output, "manifest: {manifest_digest}");
    output
}

#[must_use]
pub fn render_transport_push_next_step(reference: &str, allow_http: bool) -> String {
    if allow_http {
        format!("Next: guild pull {reference} --allow-http")
    } else {
        format!("Next: guild pull {reference}")
    }
}

#[must_use]
pub fn render_imported_skill_review(installed: &InstalledSkill) -> String {
    let verification = installed.trust.verification_state.to_string();
    let trust = installed.trust.trust_tier.to_string();
    let publisher = installed
        .verification
        .as_ref()
        .map_or("local-source", |record| record.publisher.id.as_str());
    let mut output = String::new();
    let _ = writeln!(
        output,
        "installed {}",
        resolved_skill_ref(&installed.resolved_ref)
    );
    let _ = writeln!(output, "publisher: {publisher}");
    let _ = writeln!(
        output,
        "status: {}",
        render_trust_status_pair(None, &verification, &trust)
    );
    output
}

#[must_use]
pub fn render_import_next_step(installed: &[InstalledSkill]) -> String {
    match installed {
        [skill] => format!(
            "Next: guild verify -v {}",
            resolved_skill_ref(&skill.resolved_ref)
        ),
        _ => "Next: guild ls skills".to_owned(),
    }
}

#[must_use]
pub fn render_trust_add_success(publisher: &TrustedPublisherRecord) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "trusted publisher {}", publisher.publisher.id);
    append_trusted_publisher_details(&mut output, publisher);
    output
}

#[must_use]
pub fn render_trusted_publisher(publisher: &TrustedPublisherRecord) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "publisher: {}", publisher.publisher.id);
    append_trusted_publisher_details(&mut output, publisher);
    output
}

#[must_use]
pub fn render_trusted_publishers_list(publishers: &[TrustedPublisherRecord]) -> String {
    let mut output = String::new();
    for (index, publisher) in publishers.iter().enumerate() {
        output.push_str(&render_trusted_publisher(publisher));
        if index + 1 < publishers.len() {
            let _ = writeln!(output);
        }
    }
    output
}

#[must_use]
pub fn render_execution_show(
    record: &ExecutionRecord,
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    let support = support_summary_for_execution(record);
    let status = execution_status_label(&record.status);
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}  {}",
        paint_status_word(styler, status),
        styler.paint(Tone::Ref, short_execution_ref(record))
    );
    let _ = writeln!(
        output,
        "skill: {}",
        styler.paint(Tone::Ref, short_resolved_skill_ref(&record.resolved_skill))
    );
    let _ = writeln!(output, "policy: {}", record.policy_decision.summary);
    let _ = writeln!(
        output,
        "support: {}",
        render_support_summary(&support, styler)
    );
    let _ = writeln!(
        output,
        "runtime: {}",
        styler.paint(Tone::Type, abi_version_label(&record.provenance.abi))
    );
    if let Some(output_summary) = record.output.as_ref().map(|output| output.summary.as_str()) {
        let _ = writeln!(output, "result: {output_summary}");
    }
    if options.verbose() {
        let trust = record.policy_decision.trust_tier.to_string();
        let verification = record.policy_decision.verification_state.to_string();
        let _ = writeln!(
            output,
            "uri: {}",
            styler.paint(Tone::Ref, record.receipt.uri.as_str())
        );
        let _ = writeln!(output, "trust: {trust} / {verification}");
        if let Some(termination) = &record.termination {
            let _ = writeln!(output, "termination: {}", format_termination(termination));
        }
    }
    if options.very_verbose() {
        let _ = writeln!(
            output,
            "started: {}",
            record
                .provenance
                .started_at_utc
                .as_deref()
                .unwrap_or("unknown")
        );
        let _ = writeln!(
            output,
            "finished: {}",
            record
                .provenance
                .finished_at_utc
                .as_deref()
                .unwrap_or("unknown")
        );
    }
    output
}

#[must_use]
pub fn render_execution_show_next_step(record: &ExecutionRecord) -> String {
    format!("Next: guild why {}", record.receipt.uri)
}

#[must_use]
pub fn render_execution_why(
    record: &ExecutionRecord,
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    let summary = why_summary(record);
    let status = execution_status_label(&record.status);
    let child_refs =
        nearby_child_execution_refs(record, if options.verbose() { usize::MAX } else { 1 });
    let evidence_refs =
        nearby_evidence_refs(record, if options.verbose() { usize::MAX } else { 1 });
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}  {}",
        paint_status_word(styler, status),
        styler.paint(Tone::Ref, short_execution_ref(record))
    );
    let _ = writeln!(output, "plan: {}", paint_status_word(styler, &summary.plan));
    let _ = writeln!(
        output,
        "proof: {}",
        paint_status_word(styler, &summary.proof)
    );
    let _ = writeln!(
        output,
        "token: {}",
        paint_status_word(styler, &summary.token)
    );
    let _ = writeln!(
        output,
        "witness: {}",
        paint_status_word(styler, &summary.witness)
    );
    if !summary.reason_codes.is_empty() {
        let _ = writeln!(output, "reason: {}", summary.reason_codes.join(", "));
    }
    let _ = writeln!(output, "policy: {}", record.policy_decision.summary);
    if let Some(termination) = &record.termination {
        let _ = writeln!(output, "detail: {}", format_termination(termination));
    }
    let _ = writeln!(
        output,
        "child executions: {}",
        record.child_executions.len()
    );
    let _ = writeln!(
        output,
        "evidence records: {}",
        record.emitted_evidence.len()
    );
    let _ = writeln!(output, "authority: {}", authority_summary(record));
    let _ = writeln!(
        output,
        "requested vs granted: {}",
        requested_vs_granted_summary(record)
    );
    if options.verbose() {
        append_authority_observation_list(&mut output, record);
        append_requested_vs_granted_details(&mut output, record);
        let hints = authority_request_hints(record);
        if !hints.is_empty() {
            let _ = writeln!(output, "request hints:");
            for hint in hints {
                let _ = writeln!(output, "- {hint}");
            }
        }
        append_ref_list(&mut output, "nearby child refs", &child_refs, styler);
        append_ref_list(&mut output, "nearby evidence refs", &evidence_refs, styler);
    } else if let Some(child_ref) = child_refs.first() {
        let _ = writeln!(
            output,
            "nearby child: {}",
            styler.paint(Tone::Ref, child_ref)
        );
    } else if let Some(evidence_ref) = evidence_refs.first() {
        let _ = writeln!(
            output,
            "nearby evidence: {}",
            styler.paint(Tone::Ref, evidence_ref)
        );
    }
    if options.verbose() {
        let trust = record.policy_decision.trust_tier.to_string();
        let verification = record.policy_decision.verification_state.to_string();
        let _ = writeln!(
            output,
            "skill: {}",
            styler.paint(Tone::Ref, short_resolved_skill_ref(&record.resolved_skill))
        );
        let _ = writeln!(output, "trust: {trust} / {verification}");
    }
    output
}

#[must_use]
pub fn render_execution_why_with_lineage(
    record: &ExecutionRecord,
    lineage: Option<&WhyLineage>,
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let mut output = render_execution_why(record, options, stream);
    if let Some(lineage) = lineage {
        append_execution_lineage(&mut output, lineage, options, stream);
    }
    output
}

#[must_use]
pub fn render_run_status(
    record: &ExecutionRecord,
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    let status = execution_status_label(&record.status);
    let proof = overall_support_word(&support_summary_for_execution(record));
    let mut output = String::new();
    let _ = write!(
        output,
        "{}  {}  {}  {}",
        paint_status_word(styler, status),
        paint_status_word(styler, proof),
        styler.paint(Tone::Ref, short_execution_ref(record)),
        styler.paint(Tone::Ref, short_resolved_skill_ref(&record.resolved_skill))
    );
    if let Some(code) = reason_codes(record).first() {
        let _ = write!(output, "  {code}");
    }
    output
}

#[must_use]
pub fn render_run_next_steps(record: &ExecutionRecord) -> Option<String> {
    if !matches!(record.status, ExecutionStatus::Succeeded) {
        return None;
    }

    let mut lines = Vec::new();
    if has_requested_vs_granted_changes(record) || has_blocked_authority_observations(record) {
        lines.push(format!("Next: guild why -v {}", record.receipt.uri));
    }
    lines.push(format!("Next: guild why {}", record.receipt.uri));
    lines.push(format!("Next: guild get {}", record.receipt.uri));
    Some(lines.join("\n"))
}

#[must_use]
pub fn render_skills_list(
    skills: &[InstalledSkill],
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    if skills.is_empty() {
        return "none\n".into();
    }

    let mut output = String::new();
    for skill in skills {
        let support = support_summary_for_skill(skill);
        let verification = skill.trust.verification_state.to_string();
        let trust = skill.trust.trust_tier.to_string();
        let _ = writeln!(
            output,
            "{}  {}  {}  {}",
            styler.paint(Tone::Ref, short_skill_ref(skill)),
            paint_status_word(styler, &verification),
            paint_status_word(styler, &trust),
            paint_status_word(styler, overall_support_word(&support))
        );
    }
    output
}

#[must_use]
pub fn render_runs_list(
    records: &[ExecutionRecord],
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    if records.is_empty() {
        return "none\n".into();
    }

    let mut output = String::new();
    for record in records {
        let _ = writeln!(
            output,
            "{}  {}  {}",
            paint_status_word(styler, execution_status_label(&record.status)),
            styler.paint(Tone::Ref, short_execution_ref(record)),
            styler.paint(Tone::Ref, short_resolved_skill_ref(&record.resolved_skill))
        );
    }
    output
}

#[must_use]
pub fn render_why_next_step(record: &ExecutionRecord) -> String {
    let mut lines = vec![format!("Next: guild get {}", record.receipt.uri)];
    if let Some(child) = record.child_executions.first() {
        lines.push(format!("Next: guild why {}", child.uri));
    } else if let Some(evidence) = record.emitted_evidence.first() {
        lines.push(format!("Next: guild show {}", evidence.uri));
    }
    lines.join("\n")
}

#[must_use]
pub fn render_evidence_list(
    records: &[EvidenceRecord],
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    if records.is_empty() {
        return "none\n".into();
    }

    let mut output = String::new();
    for record in records {
        let _ = writeln!(
            output,
            "{}  {}  {}",
            styler.paint(Tone::Ref, short_evidence_ref(record)),
            record.mime_type,
            format_bytes(record.size_bytes)
        );
    }
    output
}

#[must_use]
pub fn render_objects_list(
    records: &[EvidenceBlobRecord],
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    if records.is_empty() {
        return "none\n".into();
    }

    let mut output = String::new();
    for record in records {
        let _ = writeln!(
            output,
            "{}  {}",
            styler.paint(Tone::Ref, short_object_ref(record)),
            format_bytes(record.size_bytes)
        );
    }
    output
}

#[must_use]
pub fn render_evidence_show(
    record: &EvidenceRecord,
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}",
        styler.paint(Tone::Ref, short_evidence_ref(record))
    );
    let _ = writeln!(output, "mime: {}", record.mime_type);
    let _ = writeln!(output, "size: {}", format_bytes(record.size_bytes));
    let _ = writeln!(
        output,
        "audience: {}  redaction: {}",
        evidence_audience_label(&record.audience),
        redaction_class_label(&record.redaction)
    );
    if let Some(execution) = &record.produced_by_execution {
        let _ = writeln!(
            output,
            "source: {}",
            styler.paint(Tone::Ref, short_prefixed_id("exec", execution))
        );
    }
    if options.verbose() {
        let _ = writeln!(
            output,
            "uri: {}",
            styler.paint(Tone::Ref, record.uri.as_str())
        );
        let _ = writeln!(
            output,
            "blob: {}",
            styler.paint(Tone::Ref, short_hash(&record.sha256))
        );
    }
    output
}

#[must_use]
pub fn render_evidence_show_next_step(record: &EvidenceRecord) -> Option<String> {
    record
        .produced_by_execution
        .as_deref()
        .map(|execution| format!("Next: guild why {}", execution_uri(execution)))
}

#[must_use]
pub fn render_object_show(
    record: &EvidenceBlobRecord,
    options: PresentationOptions,
    stream: StreamKind,
) -> String {
    let styler = options.styler(stream);
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}",
        styler.paint(Tone::Ref, short_object_ref(record))
    );
    let _ = writeln!(output, "size: {}", format_bytes(record.size_bytes));
    if options.verbose() {
        let _ = writeln!(
            output,
            "uri: {}",
            styler.paint(Tone::Ref, record.uri.as_str())
        );
        let _ = writeln!(
            output,
            "sha256: {}",
            styler.paint(Tone::Ref, record.sha256.as_str())
        );
    }
    output
}

#[must_use]
pub fn render_skill_porcelain(installed: &InstalledSkill) -> String {
    let support = support_summary_for_skill(installed);
    format!(
        "skill\t{}\t{}\t{}\t{}",
        short_skill_ref(installed),
        installed.trust.verification_state,
        installed.trust.trust_tier,
        overall_support_word(&support)
    )
}

#[must_use]
pub fn render_verify_porcelain(installed: &InstalledSkill) -> String {
    format!(
        "verify\t{}\t{}\t{}",
        short_skill_ref(installed),
        installed.trust.verification_state,
        installed.trust.trust_tier,
    )
}

#[must_use]
pub fn render_run_porcelain(record: &ExecutionRecord) -> String {
    format!(
        "run\t{}\t{}\t{}\t{}",
        execution_status_label(&record.status),
        overall_support_word(&support_summary_for_execution(record)),
        record.receipt.execution_id,
        short_resolved_skill_ref(&record.resolved_skill)
    )
}

#[must_use]
pub fn render_why_porcelain(record: &ExecutionRecord) -> String {
    let why = why_summary(record);
    format!(
        "why\t{}\t{}\t{}\t{}\t{}\t{}",
        record.receipt.execution_id,
        why.plan,
        why.proof,
        why.token,
        why.witness,
        why.reason_codes.join(",")
    )
}

fn append_execution_lineage(
    output: &mut String,
    lineage: &WhyLineage,
    options: PresentationOptions,
    stream: StreamKind,
) {
    let styler = options.styler(stream);
    let _ = writeln!(output);
    let _ = writeln!(output, "lineage:");
    if lineage.ancestry.is_empty() {
        let _ = writeln!(output, "ancestry: none");
    } else {
        let _ = writeln!(output, "ancestry:");
        for record in &lineage.ancestry {
            append_lineage_record(output, record, 0, None, options, styler);
        }
    }
    let _ = writeln!(output, "descendants:");
    for node in &lineage.descendants {
        append_lineage_record(
            output,
            &node.record,
            node.depth,
            node.alias_from_parent.as_deref(),
            options,
            styler,
        );
    }
    if lineage.warnings.is_empty() {
        return;
    }
    if !options.verbose() {
        let _ = writeln!(
            output,
            "lineage warnings: {} (use -v to inspect)",
            lineage.warnings.len()
        );
        return;
    }

    let _ = writeln!(output, "lineage warnings:");
    for warning in &lineage.warnings {
        let mut line = format!(
            "- {} / {} / depth {}",
            warning.relation, warning.code, warning.depth
        );
        if let Some(uri) = warning.execution_uri.as_deref() {
            let location = if options.very_verbose() {
                uri.to_owned()
            } else {
                display_resource_ref_or_uri(uri)
            };
            line.push_str(" / ");
            line.push_str(&styler.paint(Tone::Ref, location));
        }
        line.push_str(" / ");
        line.push_str(&warning.message);
        if let Some(detail) = warning.detail.as_deref() {
            line.push_str(" / ");
            line.push_str(detail);
        }
        let _ = writeln!(output, "{line}");
    }
}

fn append_lineage_record(
    output: &mut String,
    record: &ExecutionRecord,
    depth: usize,
    alias_from_parent: Option<&str>,
    options: PresentationOptions,
    styler: Styler,
) {
    let indent = "  ".repeat(depth);
    let mut line = format!("{indent}- ");
    if let Some(alias) = alias_from_parent {
        line.push_str("alias ");
        line.push_str(alias);
        line.push_str("  ");
    }
    line.push_str(&paint_status_word(
        styler,
        execution_status_label(&record.status),
    ));
    line.push_str("  ");
    line.push_str(&styler.paint(Tone::Ref, short_execution_ref(record)));
    line.push_str("  ");
    line.push_str(&styler.paint(Tone::Ref, short_resolved_skill_ref(&record.resolved_skill)));
    let _ = write!(
        line,
        "  child {}  evidence {}",
        record.child_executions.len(),
        record.emitted_evidence.len()
    );
    if options.verbose() {
        let reasons = reason_codes(record);
        if !reasons.is_empty() {
            line.push_str("  reason ");
            line.push_str(&reasons.join(", "));
        }
    }
    let _ = writeln!(output, "{line}");
    if options.very_verbose() {
        let _ = writeln!(
            output,
            "{}  uri: {}",
            indent,
            styler.paint(Tone::Ref, record.receipt.uri.as_str())
        );
    }
}

fn support_summary_for_capabilities<'a>(
    capabilities: impl IntoIterator<Item = &'a CapabilityId>,
) -> SupportSummary {
    let mut proof_backed = Vec::new();
    let mut bounded = Vec::new();
    let mut not_proven = Vec::new();
    let mut refused = Vec::new();

    for capability in capabilities {
        match capability {
            CapabilityId::LogWrite => proof_backed.push(capability_id_label(capability).to_owned()),
            CapabilityId::HttpRequest | CapabilityId::ReadResource | CapabilityId::InvokeSkill => {
                bounded.push(capability_id_label(capability).to_owned());
            }
            CapabilityId::EmitEvidence => {
                not_proven.push(capability_id_label(capability).to_owned());
            }
            _ => refused.push(capability_id_label(capability).to_owned()),
        }
    }

    let mut buckets = Vec::new();
    if !proof_backed.is_empty() {
        buckets.push(SupportBucket {
            status: PRESENTATION_STATUS_PROOF_BACKED.into(),
            capabilities: proof_backed,
        });
    }
    if !bounded.is_empty() {
        buckets.push(SupportBucket {
            status: SUPPORT_STATUS_BOUNDED.into(),
            capabilities: bounded,
        });
    }
    if !not_proven.is_empty() {
        buckets.push(SupportBucket {
            status: SUPPORT_STATUS_NOT_PROVEN.into(),
            capabilities: not_proven,
        });
    }
    if !refused.is_empty() {
        buckets.push(SupportBucket {
            status: PRESENTATION_STATUS_REFUSED.into(),
            capabilities: refused,
        });
    }

    let overall = overall_support_word_from_buckets(&buckets).into();
    SupportSummary { overall, buckets }
}

fn observed_capability_ids(record: &ExecutionRecord) -> Vec<&CapabilityId> {
    let mut capabilities = Vec::new();
    for observation in &record.authority_observations {
        match observation {
            AuthorityObservation::HttpRequest {
                status: AuthorityObservationStatus::Exercised,
                ..
            } => capabilities.push(&CapabilityId::HttpRequest),
            AuthorityObservation::ReadResource {
                status: AuthorityObservationStatus::Exercised,
                ..
            } => capabilities.push(&CapabilityId::ReadResource),
            AuthorityObservation::InvokeSkill {
                status: AuthorityObservationStatus::Exercised,
                ..
            } => capabilities.push(&CapabilityId::InvokeSkill),
            AuthorityObservation::EmitEvidence {
                status: AuthorityObservationStatus::Exercised,
                ..
            } => capabilities.push(&CapabilityId::EmitEvidence),
            AuthorityObservation::LogWrite {
                status: AuthorityObservationStatus::Exercised,
                ..
            } => capabilities.push(&CapabilityId::LogWrite),
            _ => {}
        }
    }
    capabilities
}

fn overall_support_word(summary: &SupportSummary) -> &'static str {
    overall_support_word_from_buckets(&summary.buckets)
}

fn overall_support_word_from_buckets(buckets: &[SupportBucket]) -> &'static str {
    if buckets
        .iter()
        .any(|bucket| bucket.status == PRESENTATION_STATUS_REFUSED)
    {
        PRESENTATION_STATUS_REFUSED
    } else if buckets
        .iter()
        .any(|bucket| bucket.status == SUPPORT_STATUS_NOT_PROVEN)
    {
        SUPPORT_STATUS_NOT_PROVEN
    } else if buckets
        .iter()
        .any(|bucket| bucket.status == SUPPORT_STATUS_BOUNDED)
    {
        SUPPORT_STATUS_BOUNDED
    } else {
        PRESENTATION_STATUS_PROOF_BACKED
    }
}

fn reason_codes(record: &ExecutionRecord) -> Vec<String> {
    let mut codes = Vec::new();
    if let Some(termination) = &record.termination {
        codes.push(termination.code.clone());
    }
    for reason in &record.policy_decision.reasons {
        if !codes.iter().any(|code| code == &reason.code) {
            codes.push(reason.code.clone());
        }
    }
    codes
}

fn authority_diff_groups(record: &ExecutionRecord) -> Vec<AuthorityDiffGroup> {
    #[derive(Default)]
    struct GroupedGrants {
        requested: Vec<GrantedCapability>,
        granted: Vec<GrantedCapability>,
    }

    let mut grouped = BTreeMap::<(&'static str, &'static str), GroupedGrants>::new();

    for grant in &record.request.requested_capabilities.grants {
        let key = (
            capability_id_label(&grant.id),
            capability_access_label(&grant.access),
        );
        let entry = grouped.entry(key).or_default();
        entry.requested.push(grant.clone());
    }

    for grant in &record.granted_capabilities.grants {
        let key = (
            capability_id_label(&grant.id),
            capability_access_label(&grant.access),
        );
        let entry = grouped.entry(key).or_default();
        entry.granted.push(grant.clone());
    }

    grouped
        .into_values()
        .filter_map(|mut entry| {
            let (id, access) = entry
                .requested
                .first()
                .or(entry.granted.first())
                .map(|grant| (grant.id.clone(), grant.access.clone()))?;
            entry.requested.sort_by_cached_key(canonical_grant_json);
            entry.granted.sort_by_cached_key(canonical_grant_json);
            let change = if entry.requested.is_empty() && !entry.granted.is_empty() {
                AuthorityDiffChange::GrantedOnly
            } else if !entry.requested.is_empty() && entry.granted.is_empty() {
                AuthorityDiffChange::RequestedOnly
            } else if canonical_grants(&entry.requested) == canonical_grants(&entry.granted) {
                AuthorityDiffChange::Same
            } else {
                AuthorityDiffChange::Changed
            };

            Some(AuthorityDiffGroup {
                id,
                access,
                requested: entry.requested,
                granted: entry.granted,
                change,
            })
        })
        .collect()
}

fn requested_vs_granted_summary(record: &ExecutionRecord) -> String {
    let mut reduced = Vec::new();
    let mut denied = Vec::new();
    let mut added = Vec::new();

    for group in authority_diff_groups(record) {
        let family = capability_id_label(&group.id);
        match group.change {
            AuthorityDiffChange::Same => {}
            AuthorityDiffChange::Changed => push_unique(&mut reduced, family),
            AuthorityDiffChange::RequestedOnly => push_unique(&mut denied, family),
            AuthorityDiffChange::GrantedOnly => push_unique(&mut added, family),
        }
    }

    if reduced.is_empty() && denied.is_empty() && added.is_empty() {
        return "unchanged".into();
    }

    let mut parts = Vec::new();
    if !reduced.is_empty() {
        parts.push(format!("reduced({})", reduced.join(", ")));
    }
    if !denied.is_empty() {
        parts.push(format!("denied({})", denied.join(", ")));
    }
    if !added.is_empty() {
        parts.push(format!("added({})", added.join(", ")));
    }
    parts.join(" ")
}

fn has_requested_vs_granted_changes(record: &ExecutionRecord) -> bool {
    authority_diff_groups(record)
        .into_iter()
        .any(|group| group.change != AuthorityDiffChange::Same)
}

fn append_requested_vs_granted_details(output: &mut String, record: &ExecutionRecord) {
    let groups = authority_diff_groups(record)
        .into_iter()
        .filter(|group| group.change != AuthorityDiffChange::Same)
        .collect::<Vec<_>>();
    if groups.is_empty() {
        return;
    }

    let _ = writeln!(output, "requested vs granted:");
    for group in groups {
        let label = format!(
            "{}/{}",
            capability_id_label(&group.id),
            capability_access_label(&group.access)
        );
        match group.change {
            AuthorityDiffChange::Changed => {
                let _ = writeln!(
                    output,
                    "- reduced {label}: requested {} -> granted {}",
                    render_grant_group(&group.requested),
                    render_grant_group(&group.granted)
                );
            }
            AuthorityDiffChange::RequestedOnly => {
                let _ = writeln!(
                    output,
                    "- denied {label}: requested {} -> granted none",
                    render_grant_group(&group.requested)
                );
            }
            AuthorityDiffChange::GrantedOnly => {
                let _ = writeln!(
                    output,
                    "- added {label}: requested none -> granted {}",
                    render_grant_group(&group.granted)
                );
            }
            AuthorityDiffChange::Same => {}
        }
    }
}

fn render_grant_group(grants: &[GrantedCapability]) -> String {
    grants
        .iter()
        .map(render_grant_constraints)
        .collect::<Vec<_>>()
        .join(" | ")
}

#[allow(clippy::too_many_lines)]
fn render_grant_constraints(grant: &GrantedCapability) -> String {
    match &grant.constraints {
        CapabilityConstraints::None(_) => "any".into(),
        CapabilityConstraints::ReadResource(constraints) => {
            let mut parts = Vec::new();
            push_part(
                &mut parts,
                option_vec_summary(
                    "uri_prefixes",
                    constraints.uri_prefixes.as_deref(),
                    Some("any"),
                    std::borrow::ToOwned::to_owned,
                ),
            );
            push_part(
                &mut parts,
                option_vec_summary(
                    "resource_kinds",
                    constraints.resource_kinds.as_deref(),
                    Some("any"),
                    |kind| resource_kind_label(kind).to_owned(),
                ),
            );
            parts.join(" ")
        }
        CapabilityConstraints::InvokeDependency(constraints) => option_vec_summary(
            "aliases",
            constraints.aliases.as_deref(),
            Some("any-declared"),
            std::borrow::ToOwned::to_owned,
        ),
        CapabilityConstraints::EmitEvidence(constraints) => {
            let mut parts = Vec::new();
            if let Some(max_bytes) = constraints.max_bytes {
                parts.push(format!("max_bytes<={max_bytes}"));
            }
            push_part(
                &mut parts,
                option_vec_summary(
                    "audiences",
                    constraints.audiences.as_deref(),
                    Some("any"),
                    |audience| evidence_audience_label(audience).to_owned(),
                ),
            );
            push_part(
                &mut parts,
                option_vec_summary(
                    "redactions",
                    constraints.redactions.as_deref(),
                    Some("any"),
                    |redaction| redaction_class_label(redaction).to_owned(),
                ),
            );
            parts.join(" ")
        }
        CapabilityConstraints::Log(constraints) => option_vec_summary(
            "levels",
            constraints.levels.as_deref(),
            Some("any"),
            |level| severity_label(level).to_owned(),
        ),
        CapabilityConstraints::HttpRequest(constraints) => {
            let mut parts = Vec::new();
            push_part(
                &mut parts,
                option_vec_summary(
                    "schemes",
                    constraints.allowed_schemes.as_deref(),
                    None,
                    |scheme| http_scheme_label(scheme).to_owned(),
                ),
            );
            push_part(
                &mut parts,
                option_vec_summary(
                    "hosts",
                    constraints.allowed_hosts.as_deref(),
                    None,
                    std::borrow::ToOwned::to_owned,
                ),
            );
            push_part(
                &mut parts,
                option_vec_summary(
                    "host_suffixes",
                    constraints.allowed_host_suffixes.as_deref(),
                    None,
                    std::borrow::ToOwned::to_owned,
                ),
            );
            push_part(
                &mut parts,
                option_vec_summary(
                    "ports",
                    constraints.allowed_ports.as_deref(),
                    None,
                    std::string::ToString::to_string,
                ),
            );
            push_part(
                &mut parts,
                option_vec_summary(
                    "methods",
                    constraints.allowed_methods.as_deref(),
                    None,
                    |method| http_method_label(method).to_owned(),
                ),
            );
            push_part(
                &mut parts,
                option_vec_summary(
                    "paths",
                    constraints.allowed_path_prefixes.as_deref(),
                    None,
                    std::borrow::ToOwned::to_owned,
                ),
            );
            if let Some(max_timeout_ms) = constraints.max_timeout_ms {
                parts.push(format!("timeout<={max_timeout_ms}"));
            }
            if let Some(max_response_bytes) = constraints.max_response_bytes {
                parts.push(format!("response_bytes<={max_response_bytes}"));
            }
            if let Some(follow_redirects) = constraints.follow_redirects {
                parts.push(format!(
                    "redirects={}",
                    if follow_redirects { "yes" } else { "no" }
                ));
            }
            if let Some(max_redirects) = constraints.max_redirects {
                parts.push(format!("max_redirects<={max_redirects}"));
            }
            append_bool_part(&mut parts, "loopback", constraints.allow_loopback);
            append_bool_part(&mut parts, "link_local", constraints.allow_link_local);
            append_bool_part(
                &mut parts,
                "private_networks",
                constraints.allow_private_networks,
            );
            append_bool_part(&mut parts, "ip_literals", constraints.allow_ip_literals);
            if parts.is_empty() {
                "any".into()
            } else {
                parts.join(" ")
            }
        }
        CapabilityConstraints::Filesystem(constraints) => canonical_value(constraints),
    }
}

fn append_bool_part(parts: &mut Vec<String>, label: &str, value: Option<bool>) {
    if let Some(value) = value {
        parts.push(format!("{label}={}", if value { "yes" } else { "no" }));
    }
}

fn option_vec_summary<T>(
    label: &str,
    values: Option<&[T]>,
    fallback: Option<&str>,
    render: impl Fn(&T) -> String,
) -> String {
    match values {
        Some(values) => format!(
            "{label}={}",
            values.iter().map(render).collect::<Vec<_>>().join(",")
        ),
        None => fallback.map_or_else(String::new, |fallback| format!("{label}={fallback}")),
    }
}

fn canonical_grants(grants: &[GrantedCapability]) -> Vec<String> {
    grants.iter().map(canonical_grant_json).collect()
}

fn canonical_grant_json(grant: &GrantedCapability) -> String {
    serde_json::to_string(grant).unwrap_or_else(|_| "null".into())
}

fn canonical_value(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

fn has_blocked_authority_observations(record: &ExecutionRecord) -> bool {
    record.authority_observations.iter().any(|observation| {
        authority_observation_status(observation) == &AuthorityObservationStatus::Blocked
    })
}

fn authority_request_hints(record: &ExecutionRecord) -> Vec<String> {
    let mut hints = Vec::new();

    if let Some(termination) = &record.termination {
        push_authority_request_hint(&mut hints, &termination.code, termination.detail.as_ref());
    }

    for reason in &record.policy_decision.reasons {
        push_authority_request_hint(&mut hints, &reason.code, reason.detail.as_ref());
    }

    for observation in &record.authority_observations {
        if authority_observation_status(observation) != &AuthorityObservationStatus::Blocked {
            continue;
        }
        let Some((code, detail)) = blocked_authority_observation_failure(observation) else {
            continue;
        };
        push_authority_request_hint(&mut hints, code, detail);
    }

    hints
}

fn push_authority_request_hint(hints: &mut Vec<String>, code: &str, detail: Option<&Value>) {
    if let Some(hint) = authority_request_hint_for_error(code, detail)
        && !hints.iter().any(|existing| existing == &hint)
    {
        hints.push(hint);
    }
}

fn blocked_authority_observation_failure(
    observation: &AuthorityObservation,
) -> Option<(&str, Option<&Value>)> {
    match observation {
        AuthorityObservation::HttpRequest { detail, .. } => detail
            .denial
            .as_ref()
            .map(|failure| (failure.code.as_str(), failure.detail.as_ref())),
        AuthorityObservation::ReadResource { detail, .. } => detail
            .denial
            .as_ref()
            .map(|failure| (failure.code.as_str(), failure.detail.as_ref())),
        AuthorityObservation::InvokeSkill { detail, .. } => detail
            .denial
            .as_ref()
            .map(|failure| (failure.code.as_str(), failure.detail.as_ref())),
        AuthorityObservation::EmitEvidence { detail, .. } => detail
            .denial
            .as_ref()
            .map(|failure| (failure.code.as_str(), failure.detail.as_ref())),
        AuthorityObservation::LogWrite { detail, .. } => detail
            .denial
            .as_ref()
            .map(|failure| (failure.code.as_str(), failure.detail.as_ref())),
    }
}

#[allow(clippy::too_many_lines)]
pub fn authority_request_hint_for_error(code: &str, detail: Option<&Value>) -> Option<String> {
    match code {
        "policy-denied" => detail
            .and_then(|detail| detail.get("reasons").and_then(Value::as_array))
            .and_then(|reasons| {
                reasons.iter().find_map(|reason| {
                    authority_request_hint_for_error(
                        reason.get("code").and_then(Value::as_str)?,
                        reason.get("detail"),
                    )
                })
            }),
        "policy-requested-capability-invalid" => Some(
            "fix the requested grant JSON so each family uses a valid typed constraint shape before rerunning".into(),
        ),
        "policy-required-capability-missing" => {
            let requirement = detail
                .and_then(|detail| detail.get("missing").and_then(Value::as_array))
                .and_then(|missing| missing.first())
                .and_then(requirement_selector_label)
                .unwrap_or_else(|| "the skill's required authority".into());
            Some(format!(
                "request {requirement} and confirm the declared required surface with `guild show -v <skill-ref>` before rerunning"
            ))
        }
        "read-resource-not-granted" | "read-resource-kind-denied" => {
            let prefix = detail
                .and_then(|detail| detail.get("uri").and_then(Value::as_str))
                .and_then(canonical_uri_prefix_for)
                .unwrap_or("guild://executions/");
            let kind = detail
                .and_then(|detail| detail.get("resource_kind").and_then(Value::as_str))
                .unwrap_or("execution");
            Some(format!(
                "request a `read-resource` `read` grant with `uri_prefixes` including `{prefix}` and `resource_kinds` including `{kind}`"
            ))
        }
        "dependency-invoke-not-granted" => {
            let alias = detail
                .and_then(|detail| detail.get("alias").and_then(Value::as_str))
                .unwrap_or("<alias>");
            Some(format!(
                "request an `invoke-skill` `invoke` grant with `aliases` including `{alias}`"
            ))
        }
        "child-capability-mismatch" => {
            let requirement = detail
                .and_then(requirement_selector_label)
                .unwrap_or_else(|| "the child skill's required authority".into());
            Some(format!(
                "expand the parent request so it covers {requirement}, then compare the parent and child declared capabilities with `guild show -v <skill-ref>`"
            ))
        }
        "emit-evidence-not-granted" => Some(
            "request an `emit-evidence` `write` grant with a bounded `max_bytes` plus explicit `audiences` and `redactions`".into(),
        ),
        "emit-evidence-too-large" => {
            let max_bytes = detail
                .and_then(|detail| detail.get("payload_bytes").and_then(Value::as_u64))
                .map_or_else(|| "the emitted payload size".into(), |bytes| bytes.to_string());
            Some(format!(
                "raise `emit-evidence.max_bytes` so it safely covers {max_bytes} bytes, or shrink the emitted payload"
            ))
        }
        "emit-evidence-audience-not-granted" => {
            let audience = detail
                .and_then(|detail| detail.get("audience").and_then(Value::as_str))
                .unwrap_or("the needed audience");
            Some(format!(
                "request an `emit-evidence` grant whose `audiences` includes `{audience}`"
            ))
        }
        "emit-evidence-redaction-not-granted" => {
            let redaction = detail
                .and_then(|detail| detail.get("redaction").and_then(Value::as_str))
                .unwrap_or("the needed redaction");
            Some(format!(
                "request an `emit-evidence` grant whose `redactions` includes `{redaction}`"
            ))
        }
        "log-write-not-granted" => Some(
            "request a `log-write` `write` grant with only the log levels the skill actually needs".into(),
        ),
        "log-level-not-granted" => {
            let level = detail
                .and_then(|detail| detail.get("level").and_then(Value::as_str))
                .unwrap_or("the needed level");
            Some(format!(
                "request a `log-write` grant whose `levels` includes `{level}`"
            ))
        }
        "http-request-not-granted" => Some(
            "request an `http-request` `read` grant with the narrow scheme, host, method, path, and destination-class limits the call actually needs".into(),
        ),
        "http-request-method-not-granted" => {
            let method = detail
                .and_then(|detail| detail.get("method").and_then(Value::as_str))
                .unwrap_or("get");
            Some(format!(
                "request an `http-request` grant whose `allowed_methods` includes `{method}`"
            ))
        }
        "http-request-scheme-not-granted" => {
            let scheme = detail
                .and_then(|detail| detail.get("scheme").and_then(Value::as_str))
                .unwrap_or("http");
            Some(format!(
                "request an `http-request` grant whose `allowed_schemes` includes `{scheme}`"
            ))
        }
        "http-request-host-not-granted" => {
            let host = detail
                .and_then(|detail| detail.get("host").and_then(Value::as_str))
                .unwrap_or("<host>");
            Some(format!(
                "request an `http-request` grant whose `allowed_hosts` or `allowed_host_suffixes` covers `{host}`"
            ))
        }
        "http-request-port-not-granted" => {
            let port = detail
                .and_then(|detail| detail.get("port").and_then(Value::as_u64))
                .map_or_else(|| "<port>".into(), |port| port.to_string());
            Some(format!(
                "request an `http-request` grant whose `allowed_ports` includes `{port}`"
            ))
        }
        "http-request-path-not-granted" => {
            let path = detail
                .and_then(|detail| detail.get("path").and_then(Value::as_str))
                .unwrap_or("/");
            Some(format!(
                "request an `http-request` grant whose `allowed_path_prefixes` covers `{path}`"
            ))
        }
        "http-request-timeout-not-granted" => {
            let timeout = detail
                .and_then(|detail| detail.get("requested_timeout_ms").and_then(Value::as_u64))
                .map_or_else(|| "the needed timeout".into(), |timeout| timeout.to_string());
            Some(format!(
                "request an `http-request` grant whose `max_timeout_ms` safely covers `{timeout}`"
            ))
        }
        "http-request-ip-literal-not-granted" => Some(
            "use a hostname that fits the existing grant, or request `allow_ip_literals=true` only when raw IP targets are actually required".into(),
        ),
        "http-request-loopback-not-granted" => Some(
            "request `allow_loopback=true` only if this execution really must reach a loopback destination".into(),
        ),
        "http-request-link-local-not-granted" => Some(
            "request `allow_link_local=true` only if this execution really must reach a link-local destination".into(),
        ),
        "http-request-private-network-not-granted" => Some(
            "request `allow_private_networks=true` only if this execution really must reach a private-network destination".into(),
        ),
        "http-request-redirect-not-allowed" => Some(
            "keep redirects disabled unless needed, or request `follow_redirects=true` with a bounded `max_redirects` and destination limits that still cover the redirect target".into(),
        ),
        "http-request-redirect-target-not-granted" => Some(
            "keep redirects disabled unless needed, or request redirect authority plus host/path limits that cover the redirect target".into(),
        ),
        _ => None,
    }
}

fn requirement_selector_label(detail: &Value) -> Option<String> {
    let id = detail.get("id").and_then(Value::as_str)?;
    let access = detail.get("access").and_then(Value::as_str)?;
    Some(format!("`{id}` `{access}`"))
}

fn canonical_uri_prefix_for(uri: &str) -> Option<&'static str> {
    match GuildResourceUri::parse(uri).ok()? {
        GuildResourceUri::Execution { .. } => Some(GUILD_EXECUTION_URI_PREFIX),
        GuildResourceUri::ObjectBlob { .. } => Some(GUILD_OBJECT_BLOB_URI_PREFIX),
        GuildResourceUri::ObjectRecord { .. } | GuildResourceUri::ObjectRecordMetadata { .. } => {
            Some(GUILD_OBJECT_RECORD_URI_PREFIX)
        }
        GuildResourceUri::ExecutionQuery { .. } => Some(GUILD_EXECUTION_QUERY_URI_PREFIX),
    }
}

fn nearby_child_execution_refs(record: &ExecutionRecord, limit: usize) -> Vec<String> {
    record
        .child_executions
        .iter()
        .take(limit)
        .map(|child| prefixed_id("exec", &child.execution_id))
        .collect()
}

fn nearby_evidence_refs(record: &ExecutionRecord, limit: usize) -> Vec<String> {
    record
        .emitted_evidence
        .iter()
        .take(limit)
        .map(|evidence| {
            evidence.uri.rsplit('/').next().map_or_else(
                || evidence.uri.clone(),
                |record_id| prefixed_id("evidence", record_id),
            )
        })
        .collect()
}

fn append_ref_list(output: &mut String, label: &str, refs: &[String], styler: Styler) {
    if refs.is_empty() {
        return;
    }

    let _ = writeln!(output, "{label}:");
    for value in refs {
        let _ = writeln!(output, "- {}", styler.paint(Tone::Ref, value));
    }
}

fn authority_summary(record: &ExecutionRecord) -> String {
    if !record.authority_observations_recorded {
        return "not-recorded".into();
    }

    let mut exercised = Vec::new();
    let mut blocked = Vec::new();

    for observation in &record.authority_observations {
        let family = authority_observation_family_label(observation);
        match authority_observation_status(observation) {
            AuthorityObservationStatus::Exercised => push_unique(&mut exercised, family),
            AuthorityObservationStatus::Blocked => push_unique(&mut blocked, family),
        }
    }

    if exercised.is_empty() && blocked.is_empty() {
        return "none".into();
    }

    let mut parts = Vec::new();
    if !exercised.is_empty() {
        parts.push(format!("exercised({})", exercised.join(", ")));
    }
    if !blocked.is_empty() {
        parts.push(format!("blocked({})", blocked.join(", ")));
    }
    parts.join(" ")
}

fn append_authority_observation_list(output: &mut String, record: &ExecutionRecord) {
    if !record.authority_observations_recorded {
        let _ = writeln!(output, "authority observations: not recorded");
        return;
    }

    if record.authority_observations.is_empty() {
        return;
    }

    let _ = writeln!(output, "authority observations:");
    for observation in &record.authority_observations {
        let _ = writeln!(output, "- {}", authority_observation_line(observation));
    }
}

fn authority_observation_line(observation: &AuthorityObservation) -> String {
    match observation {
        AuthorityObservation::HttpRequest { status, detail } => {
            let mut parts = vec![detail.request.url.clone()];
            if let Some(response_status) = detail.response_status {
                parts.push(format!("status {response_status}"));
            }
            format!(
                "{} http-request -> {}",
                authority_observation_status_label(status),
                authority_observation_parts(
                    parts,
                    detail.denial.as_ref().map(|failure| failure.code.as_str()),
                    detail
                        .result_error
                        .as_ref()
                        .map(|failure| failure.code.as_str()),
                    None,
                )
            )
        }
        AuthorityObservation::ReadResource { status, detail } => format!(
            "{} read-resource -> {}",
            authority_observation_status_label(status),
            authority_observation_parts(
                vec![
                    display_resource_ref_or_uri(&detail.uri),
                    detail
                        .resource_kind
                        .as_ref()
                        .map_or("resource", resource_kind_label)
                        .into(),
                ],
                detail.denial.as_ref().map(|failure| failure.code.as_str()),
                detail
                    .result_error
                    .as_ref()
                    .map(|failure| failure.code.as_str()),
                detail.bytes.map(format_bytes),
            )
        ),
        AuthorityObservation::InvokeSkill { status, detail } => format!(
            "{} invoke-skill -> {}",
            authority_observation_status_label(status),
            {
                let mut parts = vec![format!("alias {}", detail.alias)];
                if let Some(execution_id) = &detail.child_execution_id {
                    parts.push(prefixed_id("exec", execution_id));
                }
                authority_observation_parts(
                    parts,
                    detail.denial.as_ref().map(|failure| failure.code.as_str()),
                    detail
                        .result_error
                        .as_ref()
                        .map(|failure| failure.code.as_str()),
                    None,
                )
            }
        ),
        AuthorityObservation::EmitEvidence { status, detail } => format!(
            "{} emit-evidence -> {}",
            authority_observation_status_label(status),
            authority_observation_parts(
                vec![
                    detail
                        .evidence_uri
                        .as_deref()
                        .map_or_else(|| detail.mime_type.clone(), display_resource_ref_or_uri),
                    format_bytes(detail.size_bytes),
                ],
                detail.denial.as_ref().map(|failure| failure.code.as_str()),
                detail
                    .result_error
                    .as_ref()
                    .map(|failure| failure.code.as_str()),
                None,
            )
        ),
        AuthorityObservation::LogWrite { status, detail } => format!(
            "{} log-write -> {}",
            authority_observation_status_label(status),
            authority_observation_parts(
                vec![severity_label(&detail.level).into()],
                detail.denial.as_ref().map(|failure| failure.code.as_str()),
                None,
                None,
            )
        ),
    }
}

fn authority_observation_parts(
    mut parts: Vec<String>,
    denial_code: Option<&str>,
    result_error_code: Option<&str>,
    trailing: Option<String>,
) -> String {
    if let Some(code) = denial_code.or(result_error_code) {
        parts.push(code.into());
    } else if let Some(trailing) = trailing {
        parts.push(trailing);
    }
    parts.join(" / ")
}

fn authority_observation_family_label(observation: &AuthorityObservation) -> &'static str {
    match observation {
        AuthorityObservation::HttpRequest { .. } => "http-request",
        AuthorityObservation::ReadResource { .. } => "read-resource",
        AuthorityObservation::InvokeSkill { .. } => "invoke-skill",
        AuthorityObservation::EmitEvidence { .. } => "emit-evidence",
        AuthorityObservation::LogWrite { .. } => "log-write",
    }
}

fn authority_observation_status(observation: &AuthorityObservation) -> &AuthorityObservationStatus {
    match observation {
        AuthorityObservation::HttpRequest { status, .. }
        | AuthorityObservation::ReadResource { status, .. }
        | AuthorityObservation::InvokeSkill { status, .. }
        | AuthorityObservation::EmitEvidence { status, .. }
        | AuthorityObservation::LogWrite { status, .. } => status,
    }
}

fn authority_observation_status_label(status: &AuthorityObservationStatus) -> &'static str {
    match status {
        AuthorityObservationStatus::Exercised => "exercised",
        AuthorityObservationStatus::Blocked => "blocked",
    }
}

fn display_resource_ref_or_uri(uri: &str) -> String {
    if let Some(execution_id) = uri.strip_prefix(GUILD_EXECUTION_URI_PREFIX) {
        return prefixed_id("exec", execution_id);
    }
    if let Some(record_id) = uri.strip_prefix(GUILD_OBJECT_RECORD_URI_PREFIX) {
        if record_id
            .strip_suffix(GUILD_OBJECT_RECORD_METADATA_URI_SUFFIX)
            .is_some()
        {
            return uri.into();
        }
        return prefixed_id("evidence", record_id);
    }
    if let Some(digest) = uri.strip_prefix(GUILD_OBJECT_BLOB_URI_PREFIX) {
        return prefixed_id("obj", digest);
    }
    if uri.starts_with(GUILD_EXECUTION_QUERY_URI_PREFIX) {
        return uri.into();
    }
    uri.into()
}

fn resource_kind_label(kind: &ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Execution => "execution",
        ResourceKind::Object => "object",
        ResourceKind::Query => "query",
    }
}

fn http_method_label(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Head => "head",
    }
}

fn http_scheme_label(scheme: &HttpScheme) -> &'static str {
    match scheme {
        HttpScheme::Http => "http",
        HttpScheme::Https => "https",
    }
}

fn severity_label(level: &Severity) -> &'static str {
    match level {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn push_unique<'a>(items: &mut Vec<&'a str>, value: &'a str) {
    if !items.iter().any(|existing| existing == &value) {
        items.push(value);
    }
}

fn push_part(parts: &mut Vec<String>, value: String) {
    if !value.is_empty() {
        parts.push(value);
    }
}

fn format_termination(termination: &TerminationDetail) -> String {
    format!(
        "{}:{}{}",
        execution_phase_label(&termination.phase),
        termination.code,
        if termination.retryable {
            " retryable"
        } else {
            ""
        }
    )
}

fn capability_summary(grants: &[CapabilityRequirement]) -> String {
    grants
        .iter()
        .map(|grant| {
            format!(
                "{}({}{})",
                capability_id_label(&grant.id),
                capability_access_label(&grant.access),
                if grant.required { ",required" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn short_hash(value: &str) -> String {
    value.chars().take(12).collect()
}

fn prefixed_id(prefix: &str, value: &str) -> String {
    format!("{prefix}:{value}")
}

fn short_prefixed_id(prefix: &str, value: &str) -> String {
    let short: String = value.chars().take(12).collect();
    format!("{prefix}:{short}")
}

fn execution_uri(execution_id: &str) -> String {
    format!("{GUILD_EXECUTION_URI_PREFIX}{execution_id}")
}

fn append_trusted_publisher_details(output: &mut String, publisher: &TrustedPublisherRecord) {
    let _ = writeln!(output, "tier: {}", publisher.trust_tier);
    let _ = writeln!(output, "name: {}", publisher.publisher.display_name);
    if let Some(homepage) = &publisher.publisher.homepage {
        let _ = writeln!(output, "homepage: {homepage}");
    }
}

fn render_support_summary(summary: &SupportSummary, styler: Styler) -> String {
    summary
        .buckets
        .iter()
        .map(|bucket| {
            format!(
                "{}({})",
                paint_status_word(styler, &bucket.status),
                bucket.capabilities.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use guild_registry::{
        BundleSignatureEnvelope, InstalledTrustMetadata, InstalledVerificationRecord,
        VerificationStatus,
    };
    use guild_types::{InstalledVerificationState, LocalTrustTier};
    use serde_json::{Value, json};

    fn test_options(verbosity: u8) -> PresentationOptions {
        PresentationOptions {
            verbosity,
            debug: false,
            color: ColorMode::Never,
            stdout_is_terminal: false,
            stderr_is_terminal: false,
        }
    }

    fn resolved_skill_json() -> serde_json::Value {
        json!({
            "key": {
                "namespace": "example",
                "name": "hello-inspect",
            },
            "version": "0.1.0",
            "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        })
    }

    fn installed_skill_for_transport_tests() -> InstalledSkill {
        let manifest: SkillManifest = serde_json::from_value(json!({
            "manifest_schema_version": "guild-manifest-v1",
            "skill_api_version": "guild-skill-v1",
            "key": {
                "namespace": "example",
                "name": "hello-inspect"
            },
            "version": "0.1.0",
            "display_name": "Hello Inspect",
            "description": "A tiny inspect-only example skill.",
            "runtime": {
                "kind": "wasm-component",
                "entrypoint": "guild-skill-inspect-v1",
                "guest_abi_version": "guild-skill-inspect-v1"
            },
            "interface": {
                "input_schema_uri": "./input.schema.json",
                "output_schema_uri": "./output.schema.json",
                "examples_uri": "./examples.json"
            },
            "behavior": {
                "category": "explain",
                "mutability": "read-only",
                "idempotent": true,
                "open_world": false,
                "freshness": "deterministic",
                "modes": {
                    "supported": ["inspect"],
                    "apply_requires_approval": false,
                    "apply_requires_idempotency_key": false
                }
            },
            "capabilities": [],
            "dependencies": [],
            "publisher": {
                "id": "local.example",
                "display_name": "Local Example",
                "homepage": null
            },
            "package": {
                "visibility": "private",
                "trust_tier": "local",
                "artifact_uri": "./component.wasm",
                "artifact_digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "sbom_uri": null,
                "signature_uri": null
            },
            "tests": []
        }))
        .unwrap();

        InstalledSkill {
            manifest,
            resolved_ref: serde_json::from_value(resolved_skill_json()).unwrap(),
            manifest_path: PathBuf::from("/tmp/manifest.json"),
            artifact_path: PathBuf::from("/tmp/component.wasm"),
            root_dir: PathBuf::from("/tmp/install"),
            verification: Some(InstalledVerificationRecord {
                status: VerificationStatus::Verified,
                publisher: manifest_publisher(),
                scheme: SignatureScheme::Ed25519,
                bundle_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .into(),
                signature: BundleSignatureEnvelope {
                    format_version: "guild-installed-bundle-signature-v1".into(),
                    scheme: SignatureScheme::Ed25519,
                    publisher_id: "local.example".into(),
                    bundle_sha256:
                        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                    signature_base64: "c2lnbmF0dXJl".into(),
                },
            }),
            trust: InstalledTrustMetadata {
                verification_state: InstalledVerificationState::VerifiedImport,
                trust_tier: LocalTrustTier::TrustedImported,
            },
        }
    }

    fn manifest_publisher() -> guild_manifest::PublisherRef {
        guild_manifest::PublisherRef {
            id: "local.example".into(),
            display_name: "Local Example".into(),
            homepage: None,
        }
    }

    fn child_record_json(index: usize) -> serde_json::Value {
        let execution_id = format!("child-exec-{index:04}-abcdef1234567890");
        json!({
            "alias": format!("child-{index}"),
            "execution_id": execution_id,
            "uri": execution_uri(&format!("child-exec-{index:04}-abcdef1234567890")),
            "parent_execution_id": "parent-exec-0001-abcdef1234567890",
            "trace_id": format!("trace-child-{index}"),
            "status": "succeeded",
            "policy_decision": {
                "outcome": "allowed",
                "summary": "allowed",
                "profile_name": "default",
                "trust_tier": "local-dev",
                "verification_state": "local-source",
                "reasons": [],
                "detail": null
            },
            "termination": null,
            "granted_capabilities": { "grants": [] },
            "metrics": {
                "duration_ms": 0,
                "network_requests": 0,
                "child_executions": 0,
                "cache_hits": 0,
                "cache_misses": 0
            },
            "provenance": {
                "resolved_skill": resolved_skill_json(),
                "abi": "guild-skill-inspect-v1",
                "dependency_digests": [],
                "started_at_utc": null,
                "finished_at_utc": null
            }
        })
    }

    fn evidence_record_json(index: usize) -> serde_json::Value {
        let record_id = format!("evidence-record-{index:04}-abcdef1234567890");
        let digest = format!("{index:064x}");
        json!({
            "uri": format!("{GUILD_OBJECT_RECORD_URI_PREFIX}{record_id}"),
            "blob_uri": format!("{GUILD_OBJECT_BLOB_URI_PREFIX}{digest}"),
            "mime_type": "application/json",
            "sha256": digest,
            "size_bytes": 128,
            "sink": null,
            "title": format!("evidence {index}"),
            "audience": "user",
            "redaction": "none",
            "freshness": "deterministic",
            "produced_by_execution": "parent-exec-0001-abcdef1234567890"
        })
    }

    fn execution_record_with_related_refs(
        child_count: usize,
        evidence_count: usize,
    ) -> ExecutionRecord {
        serde_json::from_value(json!({
            "receipt": {
                "execution_id": "parent-exec-0001-abcdef1234567890",
                "uri": execution_uri("parent-exec-0001-abcdef1234567890"),
                "trace_id": "trace-parent-1",
                "status": "succeeded"
            },
            "request": {
                "request_id": "request-1",
                "skill": {
                    "key": {
                        "namespace": "example",
                        "name": "hello-inspect"
                    },
                    "version_req": "^0.1"
                },
                "tenant_id": "tenant-1",
                "actor_id": "actor-1",
                "mode": "inspect",
                "input": {},
                "budget": {
                    "max_millis": 1000,
                    "max_memory_bytes": 1_048_576,
                    "max_output_bytes": 65_536,
                    "max_network_requests": 4,
                    "max_child_executions": 4
                },
                "requested_capabilities": { "grants": [] },
                "idempotency_key": null,
                "trace_id": "trace-parent-1"
            },
            "policy_decision": {
                "outcome": "allowed",
                "summary": "allowed",
                "profile_name": "default",
                "trust_tier": "local-dev",
                "verification_state": "local-source",
                "reasons": [],
                "detail": null
            },
            "resolved_skill": resolved_skill_json(),
            "parent_execution_id": null,
            "status": "succeeded",
            "output": null,
            "termination": null,
            "granted_capabilities": { "grants": [] },
            "emitted_evidence": (0..evidence_count).map(evidence_record_json).collect::<Vec<_>>(),
            "authority_observations": [],
            "metrics": {
                "duration_ms": 0,
                "network_requests": 0,
                "child_executions": child_count,
                "cache_hits": 0,
                "cache_misses": 0
            },
            "provenance": {
                "resolved_skill": resolved_skill_json(),
                "abi": "guild-skill-inspect-v1",
                "dependency_digests": [],
                "started_at_utc": null,
                "finished_at_utc": null
            },
            "child_executions": (0..child_count).map(child_record_json).collect::<Vec<_>>()
        }))
        .unwrap()
    }

    fn lineage_execution_record(
        execution_id: &str,
        parent_execution_id: Option<&str>,
        skill_name: &str,
        status: &str,
        child_count: usize,
        evidence_count: usize,
        reason_code: Option<&str>,
    ) -> ExecutionRecord {
        let mut value = serde_json::to_value(execution_record_with_related_refs(
            child_count,
            evidence_count,
        ))
        .unwrap();
        *value.pointer_mut("/receipt/execution_id").unwrap() = json!(execution_id);
        *value.pointer_mut("/receipt/uri").unwrap() = json!(execution_uri(execution_id));
        *value.pointer_mut("/receipt/trace_id").unwrap() = json!(format!("trace-{execution_id}"));
        *value.pointer_mut("/receipt/status").unwrap() = json!(status);
        *value.pointer_mut("/request/request_id").unwrap() =
            json!(format!("request-{execution_id}"));
        *value.pointer_mut("/request/trace_id").unwrap() = json!(format!("trace-{execution_id}"));
        *value.pointer_mut("/request/skill/key/name").unwrap() = json!(skill_name);
        *value.pointer_mut("/resolved_skill/key/name").unwrap() = json!(skill_name);
        *value.pointer_mut("/parent_execution_id").unwrap() =
            parent_execution_id.map_or(Value::Null, |parent| json!(parent));
        *value.pointer_mut("/status").unwrap() = json!(status);
        *value.pointer_mut("/policy_decision/summary").unwrap() = match status {
            "rejected" => json!("rejected"),
            _ => json!("allowed"),
        };
        *value.pointer_mut("/policy_decision/outcome").unwrap() = match status {
            "rejected" => json!("rejected"),
            _ => json!("allowed"),
        };
        *value.pointer_mut("/policy_decision/reasons").unwrap() = reason_code.map_or_else(
            || json!([]),
            |code| json!([{ "code": code, "message": code, "detail": null }]),
        );
        serde_json::from_value(value).unwrap()
    }

    fn legacy_execution_record_without_authority_observations() -> ExecutionRecord {
        let mut value = serde_json::to_value(execution_record_with_related_refs(0, 0)).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("authority_observations");
        object.remove("authority_observations_recorded");
        serde_json::from_value(value).unwrap()
    }

    fn execution_record_with_authority_diff(
        requested_grants: Value,
        granted_grants: Value,
        termination: Option<Value>,
    ) -> ExecutionRecord {
        let mut value = serde_json::to_value(execution_record_with_related_refs(0, 0)).unwrap();
        *value
            .pointer_mut("/request/requested_capabilities/grants")
            .unwrap() = requested_grants;
        *value.pointer_mut("/granted_capabilities/grants").unwrap() = granted_grants;
        *value.pointer_mut("/termination").unwrap() = termination.unwrap_or(Value::Null);
        serde_json::from_value(value).unwrap()
    }

    fn execution_record_with_blocked_observation(observation: &Value) -> ExecutionRecord {
        let mut value = serde_json::to_value(execution_record_with_related_refs(0, 0)).unwrap();
        *value.pointer_mut("/authority_observations").unwrap() = json!([observation]);
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn compact_why_output_prefers_one_child_ref_over_evidence() {
        let record = execution_record_with_related_refs(1, 1);

        let output = render_execution_why(&record, test_options(0), StreamKind::Stdout);

        assert!(output.contains("nearby child: exec:"), "{output}");
        assert!(!output.contains("nearby evidence: "), "{output}");
    }

    #[test]
    fn verbose_why_output_lists_all_related_refs() {
        let record = execution_record_with_related_refs(4, 4);

        let output = render_execution_why(&record, test_options(1), StreamKind::Stdout);

        assert!(output.contains("nearby child refs:"), "{output}");
        assert!(output.contains("nearby evidence refs:"), "{output}");
        for index in 0..4 {
            let child_execution_id = format!("child-exec-{index:04}-abcdef1234567890");
            let child_ref = format!("exec:{child_execution_id}");
            let evidence_id = format!("evidence-record-{index:04}-abcdef1234567890");
            let evidence_ref = format!("evidence:{evidence_id}");
            assert!(output.contains(&format!("- {child_ref}")), "{output}");
            assert!(output.contains(&format!("- {evidence_ref}")), "{output}");
        }
    }

    #[test]
    fn display_resource_ref_preserves_metadata_uris() {
        let metadata_uri = format!(
            "{GUILD_OBJECT_RECORD_URI_PREFIX}evidence-record-0001-abcdef1234567890{GUILD_OBJECT_RECORD_METADATA_URI_SUFFIX}"
        );

        assert_eq!(display_resource_ref_or_uri(&metadata_uri), metadata_uri);
    }

    #[test]
    fn invoke_skill_observation_keeps_child_ref_when_result_error_is_present() {
        let observation = AuthorityObservation::InvokeSkill {
            status: AuthorityObservationStatus::Exercised,
            detail: guild_types::InvokeSkillAuthorityObservation {
                alias: "hello".into(),
                child_execution_id: Some("child-exec-0001-abcdef1234567890".into()),
                child_status: Some(ExecutionStatus::Failed),
                denial: None,
                result_error: Some(guild_types::AuthorityObservationFailure {
                    code: "child-skill-failed".into(),
                    message: "child skill failed".into(),
                    detail: None,
                }),
            },
        };

        let line = authority_observation_line(&observation);

        assert!(
            line.contains(
                "alias hello / exec:child-exec-0001-abcdef1234567890 / child-skill-failed"
            ),
            "{line}"
        );
    }

    #[test]
    fn http_request_observation_keeps_response_status_when_result_error_is_present() {
        let observation = AuthorityObservation::HttpRequest {
            status: AuthorityObservationStatus::Exercised,
            detail: guild_types::HttpAuthorityObservation {
                request: guild_types::HttpRequest {
                    method: guild_types::HttpMethod::Get,
                    url: "http://127.0.0.1/not-json".into(),
                    timeout_ms: None,
                },
                response_status: Some(200),
                response_content_type: Some("text/plain".into()),
                response_bytes: Some(12),
                redirects_followed: Some(0),
                resolution: None,
                denial: None,
                result_error: Some(guild_types::AuthorityObservationFailure {
                    code: "http-response-not-json".into(),
                    message: "response was not valid JSON".into(),
                    detail: None,
                }),
            },
        };

        let line = authority_observation_line(&observation);

        assert!(
            line.contains("http://127.0.0.1/not-json / status 200 / http-response-not-json"),
            "{line}"
        );
    }

    #[test]
    fn legacy_records_render_authority_as_not_recorded() {
        let record = legacy_execution_record_without_authority_observations();

        let compact = render_execution_why(&record, test_options(0), StreamKind::Stdout);
        let verbose = render_execution_why(&record, test_options(1), StreamKind::Stdout);

        assert!(compact.contains("authority: not-recorded"), "{compact}");
        assert!(
            verbose.contains("authority observations: not recorded"),
            "{verbose}"
        );
    }

    #[test]
    fn why_output_includes_requested_vs_granted_summary_and_detail() {
        let record = execution_record_with_authority_diff(
            json!([
                {
                    "id": "emit-evidence",
                    "access": "write",
                    "constraints": {
                        "max_bytes": 65536,
                        "audiences": ["user"],
                        "redactions": ["none"]
                    }
                }
            ]),
            json!([
                {
                    "id": "emit-evidence",
                    "access": "write",
                    "constraints": {
                        "max_bytes": 1024,
                        "audiences": ["user"],
                        "redactions": ["none"]
                    }
                }
            ]),
            None,
        );

        let compact = render_execution_why(&record, test_options(0), StreamKind::Stdout);
        let verbose = render_execution_why(&record, test_options(1), StreamKind::Stdout);

        assert!(
            compact.contains("requested vs granted: reduced(emit-evidence)"),
            "{compact}"
        );
        assert!(
            verbose.contains("- reduced emit-evidence/write:"),
            "{verbose}"
        );
        assert!(verbose.contains("max_bytes<=65536"), "{verbose}");
        assert!(verbose.contains("max_bytes<=1024"), "{verbose}");
    }

    #[test]
    fn nested_policy_denial_hint_uses_reason_detail() {
        let hint = authority_request_hint_for_error(
            "policy-denied",
            Some(&json!({
                "reasons": [
                    {
                        "code": "read-resource-not-granted",
                        "detail": {
                            "uri": "guild://executions/exec-1",
                            "resource_kind": "execution"
                        }
                    }
                ]
            })),
        );

        assert_eq!(
            hint,
            Some(
                "request a `read-resource` `read` grant with `uri_prefixes` including `guild://executions/` and `resource_kinds` including `execution`"
                    .into()
            )
        );
    }

    #[test]
    fn redirect_not_allowed_hint_is_family_aware() {
        let hint = authority_request_hint_for_error(
            "http-request-redirect-not-allowed",
            Some(&json!({
                "url": "http://127.0.0.1:8080/redirect-json",
                "status": 302,
                "location": "/json"
            })),
        );

        assert_eq!(
            hint,
            Some(
                "keep redirects disabled unless needed, or request `follow_redirects=true` with a bounded `max_redirects` and destination limits that still cover the redirect target"
                    .into()
            )
        );
    }

    #[test]
    fn blocked_observations_add_request_hints_even_without_termination_or_policy_reasons() {
        let observation = json!({
            "family": "http-request",
            "status": "blocked",
            "detail": {
                "request": {
                    "method": "get",
                    "url": "http://127.0.0.1/blocked.json",
                    "timeout_ms": null
                },
                "denial": {
                    "code": "http-request-path-not-granted",
                    "message": "requested path is not granted",
                    "detail": {
                        "path": "/blocked.json"
                    }
                }
            }
        });
        let record = execution_record_with_blocked_observation(&observation);

        let verbose = render_execution_why(&record, test_options(1), StreamKind::Stdout);

        assert!(verbose.contains("authority observations:"), "{verbose}");
        assert!(
            verbose.contains(
                "- blocked http-request -> http://127.0.0.1/blocked.json / http-request-path-not-granted"
            ),
            "{verbose}"
        );
        assert!(verbose.contains("request hints:"), "{verbose}");
        assert!(
            verbose.contains(
                "- request an `http-request` grant whose `allowed_path_prefixes` covers `/blocked.json`"
            ),
            "{verbose}"
        );
    }

    #[test]
    fn compact_lineage_output_renders_bounded_tree_shape() {
        let root = execution_record_with_related_refs(1, 0);
        let child = lineage_execution_record(
            "child-exec-0000-abcdef1234567890",
            Some("parent-exec-0001-abcdef1234567890"),
            "hello-child",
            "succeeded",
            0,
            1,
            None,
        );
        let lineage = WhyLineage {
            ancestry: Vec::new(),
            descendants: vec![
                WhyLineageNode {
                    depth: 0,
                    alias_from_parent: None,
                    record: root.clone(),
                },
                WhyLineageNode {
                    depth: 1,
                    alias_from_parent: Some("child-0".into()),
                    record: child,
                },
            ],
            warnings: Vec::new(),
        };

        let output = render_execution_why_with_lineage(
            &root,
            Some(&lineage),
            test_options(0),
            StreamKind::Stdout,
        );

        assert!(output.contains("lineage:"), "{output}");
        assert!(output.contains("ancestry: none"), "{output}");
        assert!(output.contains("descendants:"), "{output}");
        assert!(
            output.contains(
                "- succeeded  exec:parent-exec-  example/hello-inspect@0.1.0  child 1  evidence 0"
            ),
            "{output}"
        );
        assert!(
            output.contains("  - alias child-0  succeeded  exec:child-exec-0"),
            "{output}"
        );
        assert!(!output.contains("lineage warnings:"), "{output}");
        assert!(!output.contains("uri: guild://executions/"), "{output}");
    }

    #[test]
    fn very_verbose_lineage_output_shows_ancestry_reason_and_warning_details() {
        let root = execution_record_with_related_refs(1, 0);
        let child = lineage_execution_record(
            "child-exec-0000-abcdef1234567890",
            Some("parent-exec-0001-abcdef1234567890"),
            "hello-child",
            "rejected",
            0,
            1,
            Some("grant:policy-denied"),
        );
        let lineage = WhyLineage {
            ancestry: vec![root.clone()],
            descendants: vec![WhyLineageNode {
                depth: 0,
                alias_from_parent: None,
                record: child.clone(),
            }],
            warnings: vec![WhyLineageWarning {
                relation: "descendants".into(),
                code: "child-read-failed".into(),
                message: "failed to load a persisted child execution while walking descendants"
                    .into(),
                execution_uri: Some(child.receipt.uri.clone()),
                depth: 1,
                detail: Some("resource/read: missing".into()),
            }],
        };

        let output = render_execution_why_with_lineage(
            &child,
            Some(&lineage),
            test_options(2),
            StreamKind::Stdout,
        );

        assert!(output.contains("ancestry:"), "{output}");
        assert!(
            output.contains("- succeeded  exec:parent-exec-  example/hello-inspect@0.1.0"),
            "{output}"
        );
        assert!(
            output.contains("uri: guild://executions/parent-exec-0001-abcdef1234567890"),
            "{output}"
        );
        assert!(output.contains("reason grant:policy-denied"), "{output}");
        assert!(output.contains("lineage warnings:"), "{output}");
        assert!(
            output.contains("descendants / child-read-failed / depth 1 / guild://executions/child-exec-0000-abcdef1234567890"),
            "{output}"
        );
        assert!(output.contains("resource/read: missing"), "{output}");
    }

    #[test]
    fn transport_export_summary_calls_out_shape_and_contents() {
        let output = render_transport_export_summary(
            "bundle",
            "skill://example/hello-inspect@0.1.0",
            "local.example",
            true,
            "/tmp/bundle",
        );

        assert!(output.contains("exported installed state"), "{output}");
        assert!(output.contains("transport: bundle"), "{output}");
        assert!(
            output.contains("skill: skill://example/hello-inspect@0.1.0"),
            "{output}"
        );
        assert!(output.contains("publisher: local.example"), "{output}");
        assert!(
            output.contains("contents: root skill plus dependency closure"),
            "{output}"
        );
        assert!(output.contains("output: /tmp/bundle"), "{output}");
        assert_eq!(
            render_transport_export_next_step("bundle", "/tmp/bundle"),
            "Next: guild import bundle /tmp/bundle"
        );
        assert_eq!(
            render_transport_export_next_step("oci-layout", "/tmp/layout"),
            "Next: guild import oci-layout /tmp/layout"
        );
    }

    #[test]
    fn transport_push_summary_calls_out_registry_destination() {
        let output = render_transport_push_summary(
            "skill://example/hello-inspect@0.1.0",
            "local.example",
            false,
            "127.0.0.1:5000/guild/hello:0.1.0",
            "sha256:abcdef",
        );

        assert!(output.contains("published installed state"), "{output}");
        assert!(output.contains("transport: oci-registry"), "{output}");
        assert!(output.contains("contents: root skill only"), "{output}");
        assert!(
            output.contains("reference: 127.0.0.1:5000/guild/hello:0.1.0"),
            "{output}"
        );
        assert!(output.contains("manifest: sha256:abcdef"), "{output}");
        assert_eq!(
            render_transport_push_next_step("127.0.0.1:5000/guild/hello:0.1.0", false),
            "Next: guild pull 127.0.0.1:5000/guild/hello:0.1.0"
        );
        assert_eq!(
            render_transport_push_next_step("127.0.0.1:5000/guild/hello:0.1.0", true),
            "Next: guild pull 127.0.0.1:5000/guild/hello:0.1.0 --allow-http"
        );
    }

    #[test]
    fn transport_import_summary_and_next_step_stay_compact() {
        let installed = vec![installed_skill_for_transport_tests()];
        let output = render_transport_import_summary("bundle", "/tmp/bundle", installed.len());

        assert!(output.contains("imported installed state"), "{output}");
        assert!(output.contains("transport: bundle"), "{output}");
        assert!(output.contains("source: /tmp/bundle"), "{output}");
        assert!(output.contains("installed: 1 skill"), "{output}");
        assert_eq!(
            render_import_next_step(&installed),
            "Next: guild verify -v skill://example/hello-inspect@0.1.0"
        );
        assert_eq!(
            render_import_next_step(&[
                installed_skill_for_transport_tests(),
                installed_skill_for_transport_tests()
            ]),
            "Next: guild ls skills"
        );
    }

    #[test]
    fn transport_import_preview_summary_reports_decision_and_reason() {
        let output = render_transport_import_preview_summary(
            "bundle",
            "/tmp/bundle",
            "would-refuse",
            "skill://example/hello-inspect@0.1.0",
            false,
            1,
            "local.example",
            &SignatureScheme::Ed25519,
            "sha256:abcdef",
            false,
            None,
            Some(&RegistryError::new(
                "bundle-publisher-untrusted",
                "signed bundle publisher was not trusted by the target Guild root",
            )),
            Some(&RegistryError::new(
                "bundle-publisher-untrusted",
                "signed bundle publisher was not trusted by the target Guild root",
            )),
        );

        assert!(output.contains("previewed installed state"), "{output}");
        assert!(output.contains("decision: would-refuse"), "{output}");
        assert!(output.contains("status: refused / untrusted"), "{output}");
        assert!(output.contains("bundle digest: sha256:abcdef"), "{output}");
        assert!(
            output.contains(
                "reason: bundle-publisher-untrusted: signed bundle publisher was not trusted by the target Guild root"
            ),
            "{output}"
        );
    }

    #[test]
    fn skill_verify_verbose_keeps_trust_review_fields_in_order() {
        let output = render_skill_verify(
            &installed_skill_for_transport_tests(),
            test_options(1),
            StreamKind::Stdout,
        );

        assert!(output.contains("publisher: local.example"), "{output}");
        assert!(
            output.contains("status: verified-import / trusted-imported"),
            "{output}"
        );
        assert!(output.contains("scheme: ed25519"), "{output}");
        assert!(
            output.contains("bundle digest: sha256:01234567"),
            "{output}"
        );
    }
}

fn format_bytes(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes}B"),
        1024..=1_048_575 => format_scaled_bytes(bytes, 1024, "KiB"),
        _ => format_scaled_bytes(bytes, 1_048_576, "MiB"),
    }
}

fn paint_status_word(styler: Styler, word: &str) -> String {
    match word {
        PRESENTATION_STATUS_PROOF_BACKED
        | PRESENTATION_STATUS_LINKED
        | "verified-import"
        | "trusted-imported"
        | "succeeded" => styler.paint(Tone::Success, word),
        SUPPORT_STATUS_BOUNDED
        | PRESENTATION_STATUS_UPPER_BOUND
        | PRESENTATION_STATUS_UNLINKED
        | "local-source"
        | "local-dev"
        | "restricted"
        | "partial" => styler.paint(Tone::Warning, word),
        SUPPORT_STATUS_NOT_PROVEN | PRESENTATION_STATUS_REFUSED | "rejected" | "failed" => {
            styler.paint(Tone::Danger, word)
        }
        _ => word.to_owned(),
    }
}

fn render_publisher_label(styler: Option<Styler>, publisher: &str) -> String {
    match styler {
        Some(styler) if publisher == "local-source" => styler.paint(Tone::Dim, publisher),
        Some(styler) => styler.paint(Tone::Ref, publisher),
        None => publisher.to_owned(),
    }
}

fn render_trust_status_pair(styler: Option<Styler>, verification: &str, trust: &str) -> String {
    match styler {
        Some(styler) => format!(
            "{} / {}",
            paint_status_word(styler, verification),
            paint_status_word(styler, trust)
        ),
        None => format!("{verification} / {trust}"),
    }
}

fn format_scaled_bytes(bytes: u64, unit_size: u64, suffix: &str) -> String {
    let whole = bytes / unit_size;
    let tenths = ((bytes % unit_size) * 10) / unit_size;
    format!("{whole}.{tenths}{suffix}")
}

fn capability_id_label(id: &CapabilityId) -> &'static str {
    id.as_str()
}

fn capability_access_label(access: &CapabilityAccess) -> &'static str {
    match access {
        CapabilityAccess::Read => "read",
        CapabilityAccess::Write => "write",
        CapabilityAccess::Invoke => "invoke",
    }
}

fn runtime_kind_label(kind: &RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::WasmComponent => "wasm-component",
        RuntimeKind::InProcess => "in-process",
        RuntimeKind::Process => "process",
        RuntimeKind::Container => "container",
    }
}

fn abi_version_label(abi: &AbiVersion) -> &'static str {
    match abi {
        AbiVersion::GuildSkillV1 => "guild-skill-v1",
        AbiVersion::GuildSkillInspectV1 => "guild-skill-inspect-v1",
    }
}

fn skill_category_label(category: &SkillCategory) -> &'static str {
    match category {
        SkillCategory::Inventory => "inventory",
        SkillCategory::Explain => "explain",
        SkillCategory::Playbook => "playbook",
        SkillCategory::Transform => "transform",
    }
}

fn signature_scheme_label(scheme: &SignatureScheme) -> &'static str {
    match scheme {
        SignatureScheme::Ed25519 => "ed25519",
    }
}

fn evidence_audience_label(audience: &EvidenceAudience) -> &'static str {
    match audience {
        EvidenceAudience::User => "user",
        EvidenceAudience::Assistant => "assistant",
        EvidenceAudience::Internal => "internal",
    }
}

fn redaction_class_label(redaction: &RedactionClass) -> &'static str {
    match redaction {
        RedactionClass::None => "none",
        RedactionClass::SecretsRemoved => "secrets-removed",
        RedactionClass::PiiRemoved => "pii-removed",
        RedactionClass::TenantSensitive => "tenant-sensitive",
    }
}

fn execution_phase_label(phase: &ExecutionPhase) -> &'static str {
    match phase {
        ExecutionPhase::Validation => "validation",
        ExecutionPhase::Grant => "grant",
        ExecutionPhase::Mode => "mode",
        ExecutionPhase::RuntimeLoad => "runtime-load",
        ExecutionPhase::RuntimeExec => "runtime-exec",
        ExecutionPhase::ChildInvocation => "child-invocation",
        ExecutionPhase::Persistence => "persistence",
        ExecutionPhase::SkillDomain => "skill-domain",
    }
}
