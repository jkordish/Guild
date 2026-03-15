use guild_types::{
    Budget, CapabilityGrantSet, ExecutionMode, ExecutionRequest, RequestedSkillRef, SkillKey,
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

    let _request = ExecutionRequest {
        execution_id: "exec-1".into(),
        skill: requested,
        tenant_id: "tenant-1".into(),
        actor_id: "actor-1".into(),
        mode: ExecutionMode::Inspect,
        input: serde_json::json!({}),
        budget: Budget::default(),
        grants: CapabilityGrantSet::default(),
        idempotency_key: None,
        parent_execution_id: None,
        trace_id: "trace-1".into(),
    };
}
