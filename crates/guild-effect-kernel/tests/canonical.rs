use guild_effect_kernel::canonical::{
    CanonicalError, canonical_bytes, canonical_digest, strict_from_slice,
};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::cell::Cell;

#[test]
fn absent_observation_matches_the_protocol_golden() {
    let value = serde_json::json!({
        "kind": "local-file-observation/v1",
        "body": {
            "state": "absent",
            "logicalAddress": "local-file:///canonical/path",
            "witnessId": "host-probe",
            "observedAt": "1788210000000000000"
        }
    });
    let bytes = canonical_bytes(&value).unwrap();
    assert_eq!(
        bytes,
        br#"{"body":{"logicalAddress":"local-file:///canonical/path","observedAt":"1788210000000000000","state":"absent","witnessId":"host-probe"},"kind":"local-file-observation/v1"}"#
    );
    assert_eq!(
        canonical_digest(&value).unwrap().as_str(),
        "sha256:37acdc8236b6c57c87a7d68b0ed51cf02d9a97ba78edd6d13a3b3f754000cf81"
    );
}

#[test]
fn strict_json_rejects_duplicate_members_recursively() {
    for (input, key) in [
        (&br#"{"a":1,"a":1}"#[..], "a"),
        (&br#"{"outer":{"a":1,"a":2}}"#[..], "a"),
        (&br#"[{"nested":0,"nested":1}]"#[..], "nested"),
    ] {
        assert!(
            matches!(
                strict_from_slice::<serde_json::Value>(input),
                Err(CanonicalError::DuplicateMember { key: actual }) if actual == key
            ),
            "accepted duplicate member in {input:?}"
        );
    }
}

#[test]
fn strict_json_rejects_every_number_outside_the_safe_uint_lexical_model() {
    for input in [
        &br#"{"a":-1}"#[..],
        &br#"{"a":-0}"#[..],
        &br#"{"a":1.0}"#[..],
        &br#"{"a":1e0}"#[..],
        &br#"{"a":1E3}"#[..],
        &br#"{"a":9007199254740992}"#[..],
        &br#"{"a":1e999}"#[..],
    ] {
        assert!(
            strict_from_slice::<serde_json::Value>(input).is_err(),
            "accepted inadmissible number in {input:?}"
        );
    }

    let value: serde_json::Value =
        strict_from_slice(br#"{"zero":0,"max":9007199254740991}"#).unwrap();
    assert_eq!(value["zero"], 0);
    assert_eq!(value["max"], 9_007_199_254_740_991_u64);
}

#[test]
fn strict_json_rejects_non_whitespace_trailing_bytes() {
    assert!(strict_from_slice::<serde_json::Value>(br#"{} []"#).is_err());
    assert!(strict_from_slice::<serde_json::Value>(br#"{} trailing"#).is_err());
    assert!(strict_from_slice::<serde_json::Value>(b"{}\n\t ").is_ok());
}

#[test]
fn canonicalization_rejects_outbound_numbers_outside_safe_uint() {
    let invalid_values = [
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!(9_007_199_254_740_992_u64),
    ];
    for value in invalid_values {
        assert!(matches!(
            canonical_bytes(&value),
            Err(CanonicalError::Number)
        ));
        assert!(matches!(
            canonical_digest(&value),
            Err(CanonicalError::Number)
        ));
    }

    for floating in [1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            canonical_bytes(&floating),
            Err(CanonicalError::Number)
        ));
        assert!(matches!(
            canonical_digest(&floating),
            Err(CanonicalError::Number)
        ));
    }
}

#[test]
fn outbound_number_validation_and_capture_use_one_serialization_pass() {
    struct ChangesNumber(Cell<bool>);

    impl Serialize for ChangesNumber {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            if self.0.replace(true) {
                serializer.serialize_f64(f64::NAN)
            } else {
                serializer.serialize_u64(1)
            }
        }
    }

    assert_eq!(
        canonical_bytes(&ChangesNumber(Cell::new(false))).unwrap(),
        b"1"
    );
    assert_eq!(
        canonical_digest(&ChangesNumber(Cell::new(false)))
            .unwrap()
            .as_str(),
        "sha256:6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b"
    );
}

#[test]
fn outbound_number_rejection_cannot_be_swallowed_by_a_serializer() {
    struct SwallowsInvalidNumber;

    impl Serialize for SwallowsInvalidNumber {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut sequence = serializer.serialize_seq(Some(1))?;
            let _ignored = sequence.serialize_element(&f64::NAN);
            sequence.end()
        }
    }

    assert!(matches!(
        canonical_bytes(&SwallowsInvalidNumber),
        Err(CanonicalError::Number)
    ));
}

#[test]
fn rfc_8785_property_ordering_uses_utf16_code_units() {
    let value = serde_json::json!({
        "\u{20ac}": "Euro Sign",
        "\r": "Carriage Return",
        "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
        "1": "One",
        "\u{1f600}": "Emoji: Grinning Face",
        "\u{0080}": "Control",
        "\u{00f6}": "Latin Small Letter O With Diaeresis"
    });

    assert_eq!(
        canonical_bytes(&value).unwrap(),
        "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\"ö\":\"Latin Small Letter O With Diaeresis\",\"€\":\"Euro Sign\",\"😀\":\"Emoji: Grinning Face\",\"דּ\":\"Hebrew Letter Dalet With Dagesh\"}"
            .as_bytes()
    );
}

#[test]
fn rfc_8785_string_escaping_is_minimal_and_lowercase() {
    let value = serde_json::json!({
        "string": "€$\u{000f}\nA'B\"\\\"/",
        "literals": [null, true, false]
    });

    assert_eq!(
        canonical_bytes(&value).unwrap(),
        "{\"literals\":[null,true,false],\"string\":\"€$\\u000f\\nA'B\\\"\\\\\\\"/\"}".as_bytes()
    );
}
