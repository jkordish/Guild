use guild_effect_kernel::scalar::{
    ArtifactName, ByteLength, Digest, EffectId, FieldName, Hex256, IdempotencyKey, Identifier,
    IncarnationId, LogicalAddress, RawDigest, ResourceKey, SafeUInt, U64Decimal, UnixNanoseconds,
    UnixSeconds, ValidationError, XattrName,
};
use proptest::prelude::*;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

const VALID_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000001";
const ZERO_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn scalar_boundaries_fail_closed() {
    assert!(LogicalAddress::parse(" local-file:///tmp/a").is_err());
    assert!(ArtifactName::parse(" \t ").is_err());
    assert_eq!(
        ArtifactName::parse("e\u{301}").unwrap().as_str(),
        "e\u{301}"
    );
    assert!(IdempotencyKey::parse("fifteen-chars!!").is_err());
    assert!(SafeUInt::new(9_007_199_254_740_991).is_ok());
    assert!(SafeUInt::new(9_007_199_254_740_992).is_err());
}

#[test]
fn decimal_encoding_is_canonical_and_checked() {
    assert!(serde_json::from_str::<U64Decimal>(r#""01""#).is_err());
    assert!(serde_json::from_str::<U64Decimal>("1").is_err());
    let max = U64Decimal::parse("18446744073709551615").unwrap();
    assert!(max.checked_add(1).is_err());
}

#[test]
fn digest_shaped_scalars_enforce_prefix_hex_length_and_nonzero() {
    assert_eq!(Digest::parse(VALID_DIGEST).unwrap().as_str(), VALID_DIGEST);
    assert_eq!(
        RawDigest::parse(VALID_DIGEST).unwrap().as_str(),
        VALID_DIGEST
    );
    assert_eq!(
        IncarnationId::parse(VALID_DIGEST).unwrap().as_str(),
        VALID_DIGEST
    );
    assert_eq!(
        ResourceKey::parse(VALID_DIGEST).unwrap().as_str(),
        VALID_DIGEST
    );
    assert_eq!(
        EffectId::parse(VALID_DIGEST).unwrap().as_str(),
        VALID_DIGEST
    );

    for invalid in [
        ZERO_DIGEST,
        "0000000000000000000000000000000000000000000000000000000000000001",
        "sha256:000000000000000000000000000000000000000000000000000000000000001",
        "sha256:00000000000000000000000000000000000000000000000000000000000000001",
        "sha256:000000000000000000000000000000000000000000000000000000000000000G",
    ] {
        assert!(Digest::parse(invalid).is_err(), "accepted {invalid:?}");
        assert!(RawDigest::parse(invalid).is_err(), "accepted {invalid:?}");
        assert!(
            IncarnationId::parse(invalid).is_err(),
            "accepted {invalid:?}"
        );
        assert!(ResourceKey::parse(invalid).is_err(), "accepted {invalid:?}");
        assert!(EffectId::parse(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn digest_shaped_scalars_remain_distinct_string_encoded_types() {
    assert_eq!(
        serde_json::to_string(&Digest::parse(VALID_DIGEST).unwrap()).unwrap(),
        format!(r#""{VALID_DIGEST}""#)
    );
    assert_eq!(
        serde_json::to_string(&RawDigest::parse(VALID_DIGEST).unwrap()).unwrap(),
        format!(r#""{VALID_DIGEST}""#)
    );
    assert_eq!(
        serde_json::to_string(&IncarnationId::parse(VALID_DIGEST).unwrap()).unwrap(),
        format!(r#""{VALID_DIGEST}""#)
    );
    assert_eq!(
        serde_json::to_string(&ResourceKey::parse(VALID_DIGEST).unwrap()).unwrap(),
        format!(r#""{VALID_DIGEST}""#)
    );
    assert_eq!(
        serde_json::to_string(&EffectId::parse(VALID_DIGEST).unwrap()).unwrap(),
        format!(r#""{VALID_DIGEST}""#)
    );
}

#[test]
fn hex256_accepts_exactly_unprefixed_lowercase_hex() {
    let zero = "0".repeat(64);
    let max = "f".repeat(64);
    assert_eq!(Hex256::parse(&zero).unwrap().as_str(), zero);
    assert_eq!(Hex256::parse(&max).unwrap().as_str(), max);

    for invalid in [
        "f".repeat(63),
        "f".repeat(65),
        format!("{}F", "f".repeat(63)),
        format!("sha256:{}", "f".repeat(64)),
    ] {
        assert!(Hex256::parse(&invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn identifier_is_closed_lower_kebab_case() {
    for valid in ["a", "0", "a1", "static-artifact-1"] {
        assert_eq!(Identifier::parse(valid).unwrap().as_str(), valid);
    }
    assert!(Identifier::parse(&"a".repeat(63)).is_ok());

    for invalid in ["", "-a", "a-", "a--b", "Upper", "snake_case", "with space"] {
        assert!(Identifier::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(Identifier::parse(&"a".repeat(64)).is_err());
}

#[test]
fn field_name_is_closed_lower_camel_ascii() {
    for valid in ["a", "field1", "logicalAddress", "sha256Digest"] {
        assert_eq!(FieldName::parse(valid).unwrap().as_str(), valid);
    }
    assert!(FieldName::parse(&"a".repeat(63)).is_ok());

    for invalid in ["", "Afield", "field_name", "field-name", "field name", "é"] {
        assert!(FieldName::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(FieldName::parse(&"a".repeat(64)).is_err());
}

#[test]
fn xattr_name_enforces_visible_ascii_and_preserves_bytes() {
    for valid in ["a", "user.guild", "com.apple.quarantine"] {
        assert_eq!(XattrName::parse(valid).unwrap().as_str(), valid);
    }
    assert!(XattrName::parse(&"x".repeat(255)).is_ok());

    for invalid in ["", "has space", "user=value", "nul\0byte", "é"] {
        assert!(XattrName::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(XattrName::parse(&"x".repeat(256)).is_err());
}

#[test]
fn logical_address_is_printable_ascii_without_surrounding_whitespace() {
    for valid in [
        "a",
        "local-file:///tmp/a",
        "local-file:///path with interior spaces/a",
    ] {
        assert_eq!(LogicalAddress::parse(valid).unwrap().as_str(), valid);
    }
    assert!(LogicalAddress::parse(&"x".repeat(255)).is_ok());

    for invalid in ["", " leading", "trailing ", "tab\there", "line\nbreak", "é"] {
        assert!(
            LogicalAddress::parse(invalid).is_err(),
            "accepted {invalid:?}"
        );
    }
    assert!(LogicalAddress::parse(&"x".repeat(256)).is_err());
}

#[test]
fn artifact_name_counts_unicode_scalars_and_never_normalizes() {
    let composed = ArtifactName::parse("é").unwrap();
    let decomposed = ArtifactName::parse("e\u{301}").unwrap();
    assert_eq!(composed.as_str(), "é");
    assert_eq!(decomposed.as_str(), "e\u{301}");
    assert_ne!(composed, decomposed);
    assert_eq!(ArtifactName::parse(" name ").unwrap().as_str(), " name ");
    assert!(ArtifactName::parse(&"é".repeat(255)).is_ok());
    assert!(ArtifactName::parse(&"é".repeat(256)).is_err());
    assert!(ArtifactName::parse("").is_err());
    assert!(ArtifactName::parse("\u{2003}\t ").is_err());
    assert!(ArtifactName::parse("a\0b").is_err());
}

#[test]
fn idempotency_key_enforces_visible_ascii_byte_boundaries() {
    for valid in [
        "abcdefghijklmnop",
        "Case-Sensitive-KEY",
        "equals=allowed!!!",
    ] {
        assert_eq!(IdempotencyKey::parse(valid).unwrap().as_str(), valid);
    }
    assert!(IdempotencyKey::parse(&"x".repeat(128)).is_ok());

    for invalid in [
        "x".repeat(15),
        "x".repeat(129),
        "eight eight chars".to_owned(),
        format!("{}\u{7f}", "x".repeat(16)),
        "é".repeat(16),
    ] {
        assert!(
            IdempotencyKey::parse(&invalid).is_err(),
            "accepted {invalid:?}"
        );
    }
}

#[test]
fn string_scalars_serialize_as_their_exact_validated_bytes() {
    assert_eq!(
        serde_json::to_string(&Hex256::parse(&"a".repeat(64)).unwrap()).unwrap(),
        format!(r#""{}""#, "a".repeat(64))
    );
    assert_eq!(
        serde_json::to_string(&Identifier::parse("artifact-name").unwrap()).unwrap(),
        r#""artifact-name""#
    );
    assert_eq!(
        serde_json::to_string(&FieldName::parse("logicalAddress").unwrap()).unwrap(),
        r#""logicalAddress""#
    );
    assert_eq!(
        serde_json::to_string(&XattrName::parse("user.guild").unwrap()).unwrap(),
        r#""user.guild""#
    );
    assert_eq!(
        serde_json::to_string(&LogicalAddress::parse("local-file:///tmp/a").unwrap()).unwrap(),
        r#""local-file:///tmp/a""#
    );
    assert_eq!(
        serde_json::to_string(&ArtifactName::parse("e\u{301}").unwrap()).unwrap(),
        r#""é""#
    );
    assert_eq!(
        serde_json::to_string(&IdempotencyKey::parse("Case-Sensitive-KEY").unwrap()).unwrap(),
        r#""Case-Sensitive-KEY""#
    );
}

#[test]
fn string_scalar_deserialization_reuses_validation() {
    assert!(serde_json::from_str::<Digest>(&format!(r#""{ZERO_DIGEST}""#)).is_err());
    assert!(serde_json::from_str::<RawDigest>(r#""sha256:ABC""#).is_err());
    assert!(serde_json::from_str::<Hex256>(&format!(r#""{}F""#, "a".repeat(63))).is_err());
    assert!(serde_json::from_str::<Identifier>(r#""Not-kebab""#).is_err());
    assert!(serde_json::from_str::<FieldName>(r#""not_field""#).is_err());
    assert!(serde_json::from_str::<XattrName>(r#""user=value""#).is_err());
    assert!(serde_json::from_str::<LogicalAddress>(r#"" trailing ""#).is_err());
    assert!(serde_json::from_str::<ArtifactName>(r#""   ""#).is_err());
    assert!(serde_json::from_str::<IdempotencyKey>(r#""too-short""#).is_err());
    assert!(serde_json::from_str::<IncarnationId>(&format!(r#""{ZERO_DIGEST}""#)).is_err());
    assert!(serde_json::from_str::<ResourceKey>(r#""missing-prefix""#).is_err());
    assert!(serde_json::from_str::<EffectId>(r#""sha256:ABC""#).is_err());
}

#[test]
fn all_string_scalar_families_reject_non_string_json() {
    assert_rejects_non_string::<Digest>();
    assert_rejects_non_string::<RawDigest>();
    assert_rejects_non_string::<Hex256>();
    assert_rejects_non_string::<Identifier>();
    assert_rejects_non_string::<FieldName>();
    assert_rejects_non_string::<XattrName>();
    assert_rejects_non_string::<LogicalAddress>();
    assert_rejects_non_string::<ArtifactName>();
    assert_rejects_non_string::<IdempotencyKey>();
    assert_rejects_non_string::<IncarnationId>();
    assert_rejects_non_string::<ResourceKey>();
    assert_rejects_non_string::<EffectId>();
}

#[test]
fn safe_uint_uses_json_numbers_and_revalidates_deserialization() {
    let maximum = SafeUInt::new(SafeUInt::MAX).unwrap();
    assert_eq!(maximum.get(), 9_007_199_254_740_991);
    assert_eq!(serde_json::to_string(&maximum).unwrap(), "9007199254740991");
    assert_eq!(serde_json::from_str::<SafeUInt>("0").unwrap().get(), 0);
    assert_eq!(
        serde_json::from_str::<SafeUInt>("9007199254740991")
            .unwrap()
            .get(),
        SafeUInt::MAX
    );
    assert!(serde_json::from_str::<SafeUInt>("9007199254740992").is_err());
    assert!(serde_json::from_str::<SafeUInt>(r#""1""#).is_err());
    assert!(serde_json::from_str::<SafeUInt>("-1").is_err());
    assert!(serde_json::from_str::<SafeUInt>("1.0").is_err());
}

#[test]
fn u64_decimal_rejects_every_noncanonical_shape_and_checks_both_directions() {
    for invalid in [
        "",
        "00",
        "01",
        "+1",
        "-1",
        " 1",
        "1 ",
        "1.0",
        "18446744073709551616",
    ] {
        assert!(U64Decimal::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert_eq!(U64Decimal::parse("0").unwrap().get(), 0);
    assert_eq!(
        U64Decimal::parse("18446744073709551615").unwrap().get(),
        u64::MAX
    );
    assert_eq!(U64Decimal::from_u64(41).checked_add(1).unwrap().get(), 42);
    assert_eq!(U64Decimal::from_u64(42).checked_sub(1).unwrap().get(), 41);
    assert!(U64Decimal::from_u64(0).checked_sub(1).is_err());
}

#[test]
fn decimal_backed_public_families_use_strings_and_checked_time_arithmetic() {
    let bytes = ByteLength::from_u64(u64::MAX);
    assert_eq!(bytes.get(), u64::MAX);
    assert_eq!(
        serde_json::to_string(&bytes).unwrap(),
        r#""18446744073709551615""#
    );
    assert_eq!(
        serde_json::from_str::<ByteLength>(r#""42""#).unwrap().get(),
        42
    );

    let seconds = UnixSeconds::parse("1788210000").unwrap();
    assert_eq!(seconds.get(), 1_788_210_000);
    assert_eq!(serde_json::to_string(&seconds).unwrap(), r#""1788210000""#);
    assert_eq!(seconds.checked_add(1).unwrap().get(), 1_788_210_001);
    assert!(
        UnixSeconds::parse("18446744073709551615")
            .unwrap()
            .checked_add(1)
            .is_err()
    );

    let nanoseconds = UnixNanoseconds::parse("1788210000000000000").unwrap();
    assert_eq!(nanoseconds.get(), 1_788_210_000_000_000_000);
    assert_eq!(
        serde_json::to_string(&nanoseconds).unwrap(),
        r#""1788210000000000000""#
    );
    assert_eq!(
        nanoseconds.checked_add(1).unwrap().get(),
        1_788_210_000_000_000_001
    );
    assert!(
        UnixNanoseconds::parse("18446744073709551615")
            .unwrap()
            .checked_add(1)
            .is_err()
    );

    for numeric_json in ["0", "42", "18446744073709551615"] {
        assert!(serde_json::from_str::<ByteLength>(numeric_json).is_err());
        assert!(serde_json::from_str::<UnixSeconds>(numeric_json).is_err());
        assert!(serde_json::from_str::<UnixNanoseconds>(numeric_json).is_err());
    }
    for noncanonical in [r#""01""#, r#""+1""#, r#""18446744073709551616""#] {
        assert!(serde_json::from_str::<ByteLength>(noncanonical).is_err());
        assert!(serde_json::from_str::<UnixSeconds>(noncanonical).is_err());
        assert!(serde_json::from_str::<UnixNanoseconds>(noncanonical).is_err());
    }
}

#[test]
fn validation_errors_report_closed_categories_with_context() {
    assert_eq!(
        Identifier::parse("").unwrap_err(),
        ValidationError::Empty {
            scalar: "Identifier"
        }
    );
    assert_eq!(
        IdempotencyKey::parse("short").unwrap_err(),
        ValidationError::Length {
            scalar: "IdempotencyKey",
            min: 16,
            max: 128,
            actual: 5,
        }
    );
    assert_eq!(
        LogicalAddress::parse("ab\tc").unwrap_err(),
        ValidationError::Character {
            scalar: "LogicalAddress",
            index: 2,
        }
    );
    assert_eq!(
        SafeUInt::new(9_007_199_254_740_992).unwrap_err(),
        ValidationError::Range {
            scalar: "SafeUInt",
            value: 9_007_199_254_740_992,
            max: 9_007_199_254_740_991,
        }
    );
    assert_eq!(
        Digest::parse(ZERO_DIGEST).unwrap_err(),
        ValidationError::Zero { scalar: "Digest" }
    );
    assert_eq!(
        U64Decimal::from_u64(u64::MAX).checked_add(1).unwrap_err(),
        ValidationError::Overflow {
            scalar: "U64Decimal"
        }
    );
}

proptest! {
    #[test]
    fn every_u64_round_trips_through_decimal_json_and_parse(value in any::<u64>()) {
        let decimal = U64Decimal::from_u64(value);
        let json = serde_json::to_string(&decimal).unwrap();
        prop_assert_eq!(&json, &format!(r#""{value}""#));

        let from_json: U64Decimal = serde_json::from_str(&json).unwrap();
        let from_parse = U64Decimal::parse(&value.to_string()).unwrap();
        prop_assert_eq!(from_json, decimal);
        prop_assert_eq!(from_parse, decimal);
        prop_assert_eq!(decimal.get(), value);
    }

    #[test]
    fn inserting_one_illegal_ascii_byte_invalidates_an_idempotency_key(
        illegal in prop_oneof![0u8..=32, Just(127u8)],
        index in 0usize..=16,
    ) {
        let mut candidate = "abcdefghijklmnop".to_owned();
        candidate.insert(index, char::from(illegal));
        prop_assert!(IdempotencyKey::parse(&candidate).is_err());
    }
}

fn assert_rejects_non_string<T>()
where
    T: DeserializeOwned + Debug,
{
    for json in ["null", "false", "1", "[]", "{}"] {
        assert!(
            serde_json::from_str::<T>(json).is_err(),
            "accepted non-string JSON {json}"
        );
    }
}
