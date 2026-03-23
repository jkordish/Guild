use std::fmt::Write as _;

use guild_manifest::SkillManifest;
use guild_registry::{InstalledSkill, SignatureScheme, TrustedPublisherRecord};
use guild_types::{
    AbiVersion, AuthorityObservation, AuthorityObservationStatus, CapabilityAccess, CapabilityId,
    CapabilityRequirement, EvidenceAudience, EvidenceBlobRecord, EvidenceRecord, ExecutionPhase,
    ExecutionRecord, ExecutionStatus, GUILD_EXECUTION_URI_PREFIX, PRESENTATION_STATUS_LINKED,
    PRESENTATION_STATUS_PROOF_BACKED, PRESENTATION_STATUS_REFUSED, PRESENTATION_STATUS_UNLINKED,
    PRESENTATION_STATUS_UPPER_BOUND, RedactionClass, ResolvedSkillRef, RuntimeKind,
    SUPPORT_STATUS_BOUNDED, SUPPORT_STATUS_NOT_PROVEN, SkillCategory, TerminationDetail,
    execution_status_label,
};
use serde::Serialize;

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
        render_trust_status_pair(Some(&styler), &verification, &trust)
    );
    let _ = writeln!(
        output,
        "support: {}",
        render_support_summary(&support, &styler)
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
        "status: {}",
        render_trust_status_pair(Some(&styler), &verification, &trust)
    );
    let _ = writeln!(
        output,
        "publisher: {}",
        render_publisher_label(Some(&styler), publisher)
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
            "bundle: {}",
            styler.paint(Tone::Ref, short_hash(&verification.bundle_sha256))
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
    let _ = writeln!(
        output,
        "status: {}",
        render_trust_status_pair(None, &verification, &trust)
    );
    let _ = writeln!(output, "publisher: {publisher}");
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
pub fn render_trusted_publishers_list(publishers: &[TrustedPublisherRecord]) -> String {
    let mut output = String::new();
    for (index, publisher) in publishers.iter().enumerate() {
        let _ = writeln!(output, "publisher: {}", publisher.publisher.id);
        append_trusted_publisher_details(&mut output, publisher);
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
        paint_status_word(&styler, status),
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
        render_support_summary(&support, &styler)
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
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}  {}",
        paint_status_word(&styler, status),
        styler.paint(Tone::Ref, short_execution_ref(record))
    );
    let _ = writeln!(
        output,
        "plan: {}",
        paint_status_word(&styler, &summary.plan)
    );
    let _ = writeln!(
        output,
        "proof: {}",
        paint_status_word(&styler, &summary.proof)
    );
    let _ = writeln!(
        output,
        "token: {}",
        paint_status_word(&styler, &summary.token)
    );
    let _ = writeln!(
        output,
        "witness: {}",
        paint_status_word(&styler, &summary.witness)
    );
    if !summary.reason_codes.is_empty() {
        let _ = writeln!(output, "reason: {}", summary.reason_codes.join(", "));
    }
    let _ = writeln!(output, "policy: {}", record.policy_decision.summary);
    if let Some(termination) = &record.termination {
        let _ = writeln!(output, "detail: {}", format_termination(termination));
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
        paint_status_word(&styler, status),
        paint_status_word(&styler, proof),
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

    Some(format!(
        "Next: guild why {}\nNext: guild get {}",
        record.receipt.uri, record.receipt.uri
    ))
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
            paint_status_word(&styler, &verification),
            paint_status_word(&styler, &trust),
            paint_status_word(&styler, overall_support_word(&support))
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
            paint_status_word(&styler, execution_status_label(&record.status)),
            styler.paint(Tone::Ref, short_execution_ref(record)),
            styler.paint(Tone::Ref, short_resolved_skill_ref(&record.resolved_skill))
        );
    }
    output
}

#[must_use]
pub fn render_why_next_step(record: &ExecutionRecord) -> String {
    format!("Next: guild get {}", record.receipt.uri)
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

#[allow(clippy::trivially_copy_pass_by_ref)]
fn render_support_summary(summary: &SupportSummary, styler: &Styler) -> String {
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

fn format_bytes(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes}B"),
        1024..=1_048_575 => format_scaled_bytes(bytes, 1024, "KiB"),
        _ => format_scaled_bytes(bytes, 1_048_576, "MiB"),
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn paint_status_word(styler: &Styler, word: &str) -> String {
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

fn render_publisher_label(styler: Option<&Styler>, publisher: &str) -> String {
    match styler {
        Some(styler) if publisher == "local-source" => styler.paint(Tone::Dim, publisher),
        Some(styler) => styler.paint(Tone::Ref, publisher),
        None => publisher.to_owned(),
    }
}

fn render_trust_status_pair(styler: Option<&Styler>, verification: &str, trust: &str) -> String {
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
