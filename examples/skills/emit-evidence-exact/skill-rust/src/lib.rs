use serde_json::json;
use sha2::{Digest, Sha256};
use wit_bindgen::generate;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill-inspect-v1",
});

use crate::exports::guild::skill::inspect_skill::{
    ExecutionContext, Guest, Json, SkillError, SkillOutput,
};
use crate::guild::skill::inspect_host as host;
use crate::guild::skill::inspect_types::{
    EvidenceAudience, EvidenceEmissionRequest, RedactionClass, ResolvedSkillRef,
};

const EXACT_PAYLOAD: &[u8] = br#"{"kind":"emit-evidence-exact","mode":"inspect"}"#;
const PAYLOAD_MIME_TYPE: &str = "application/json";
const PAYLOAD_TITLE: &str = "emit-evidence exact payload";

struct EmitEvidenceExact;

impl Guest for EmitEvidenceExact {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        if input.trim() != "{}" {
            return Err(SkillError {
                code: "invalid-input".into(),
                message: "emit-evidence-exact expects an empty JSON object".into(),
                retryable: false,
                detail: Some(json!({ "received": input }).to_string()),
            });
        }

        let evidence = host::emit_evidence(&EvidenceEmissionRequest {
            payload: EXACT_PAYLOAD.to_vec(),
            mime_type: PAYLOAD_MIME_TYPE.into(),
            title: Some(PAYLOAD_TITLE.into()),
            audience: EvidenceAudience::User,
            redaction: RedactionClass::None,
            freshness: Some("deterministic".into()),
        })
        .map_err(|message| SkillError {
            code: "emit-evidence-failed".into(),
            message: "host failed to persist exact emit-evidence payload".into(),
            retryable: false,
            detail: Some(json!({ "error": message }).to_string()),
        })?;

        Ok(SkillOutput {
            summary: "Exact emit-evidence fixture completed.".into(),
            structured: json!({
                "mode": "inspect",
                "skill": resolved_skill_identity(&ctx.skill),
                "payload": {
                    "mime_type": PAYLOAD_MIME_TYPE,
                    "size_bytes": EXACT_PAYLOAD.len(),
                    "sha256": exact_payload_sha256(),
                },
                "message": "emitted one exact proof fixture payload",
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: vec![evidence],
        })
    }
}

fn resolved_skill_identity(skill: &ResolvedSkillRef) -> serde_json::Value {
    json!({
        "key": {
            "namespace": skill.key.namespace,
            "name": skill.key.name,
        },
        "version": skill.version,
        "digest": skill.digest,
    })
}

fn exact_payload_sha256() -> String {
    format!("sha256:{:x}", Sha256::digest(EXACT_PAYLOAD))
}

export!(EmitEvidenceExact with_types_in self);
