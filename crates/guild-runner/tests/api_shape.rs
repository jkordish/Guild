#[test]
fn execution_request_rejects_requested_skill_refs() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/execution_request_rejects_requested_skill_ref.rs");
}
