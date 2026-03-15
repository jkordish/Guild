use guild_types::{
    Budget, CallerRequest, CapabilityGrantSet, ExecutionMode, PolicyDecision,
    PolicyDecisionOutcome, RequestedSkillRef, ResolvedExecutionEnvelope, SkillKey,
    VersionRequirement,
};

fn main() {
    let requested = RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-inspect".into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    };

    let _request = ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: "request-1".into(),
            skill: requested,
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode: ExecutionMode::Inspect,
            input: serde_json::json!({}),
            budget: Budget::default(),
            requested_capabilities: CapabilityGrantSet::default(),
            idempotency_key: None,
            trace_id: "trace-1".into(),
        },
        resolved_skill: requested,
        granted_capabilities: CapabilityGrantSet::default(),
        policy_decision: PolicyDecision {
            outcome: PolicyDecisionOutcome::Allowed,
            summary: "allowed".into(),
            detail: None,
        },
        parent_execution_id: None,
    };
}
