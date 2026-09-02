use guild_effect_kernel::protocol::{BODY_KIND_IDS, EVENT_SCHEMA_VERSION, EVENT_TYPE_IDS};

#[test]
fn protocol_v1_identifiers_are_frozen() {
    assert_eq!(EVENT_SCHEMA_VERSION, "jidoka.dev/events/v1");
    assert_eq!(BODY_KIND_IDS.len(), 29);
    assert_eq!(BODY_KIND_IDS[0], "installation-enrollment/v1");
    assert_eq!(BODY_KIND_IDS[28], "dossier-summary/v1");
    assert_eq!(EVENT_TYPE_IDS.len(), 26);
    assert_eq!(EVENT_TYPE_IDS[0], "installation_enrolled");
    assert_eq!(EVENT_TYPE_IDS[25], "custody_disputed");
}
