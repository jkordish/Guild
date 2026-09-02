use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Closed validation failures for protocol scalar values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("{scalar} must not be empty")]
    Empty { scalar: &'static str },
    #[error("{scalar} length {actual} is outside {min}..={max}")]
    Length {
        scalar: &'static str,
        min: usize,
        max: usize,
        actual: usize,
    },
    #[error("{scalar} contains a forbidden character at byte {index}")]
    Character { scalar: &'static str, index: usize },
    #[error("{scalar} is not in canonical format")]
    Format { scalar: &'static str },
    #[error("{scalar} value {value} exceeds the admitted maximum {max}")]
    Range {
        scalar: &'static str,
        value: u128,
        max: u128,
    },
    #[error("{scalar} must be nonzero")]
    Zero { scalar: &'static str },
    #[error("{scalar} arithmetic overflow")]
    Overflow { scalar: &'static str },
}

macro_rules! digest_scalar {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Parses a canonical, nonzero SHA-256 identity.
            ///
            /// # Errors
            ///
            /// Returns the closed validation category for a noncanonical value.
            pub fn parse(input: &str) -> Result<Self, ValidationError> {
                validate_digest(input, stringify!($name))?;
                Ok(Self(input.to_owned()))
            }

            /// Returns the validated wire representation.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

digest_scalar!(Digest);
digest_scalar!(RawDigest);
digest_scalar!(IncarnationId);
digest_scalar!(ResourceKey);
digest_scalar!(EffectId);

macro_rules! impl_parse_deserialize {
    ($($name:ident),+ $(,)?) => {
        $(
            impl<'de> Deserialize<'de> for $name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    let input = String::deserialize(deserializer)?;
                    Self::parse(&input).map_err(serde::de::Error::custom)
                }
            }
        )+
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Hex256(String);

impl Hex256 {
    /// Parses exactly 64 unprefixed lowercase hexadecimal digits.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        validate_lower_hex(input, "Hex256", 64, 0)?;
        Ok(Self(input.to_owned()))
    }

    /// Returns the validated wire representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Identifier(String);

impl Identifier {
    /// Parses a lower-kebab-case ASCII identifier.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        validate_nonempty_byte_length(input, "Identifier", 1, 63)?;
        let bytes = input.as_bytes();
        for (index, byte) in bytes.iter().copied().enumerate() {
            let alphanumeric = byte.is_ascii_lowercase() || byte.is_ascii_digit();
            let valid_hyphen =
                byte == b'-' && index > 0 && index + 1 < bytes.len() && bytes[index - 1] != b'-';
            if !alphanumeric && !valid_hyphen {
                return Err(ValidationError::Character {
                    scalar: "Identifier",
                    index,
                });
            }
        }
        Ok(Self(input.to_owned()))
    }

    /// Returns the validated wire representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct FieldName(String);

impl FieldName {
    /// Parses a lower-camel-case ASCII field name.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        validate_nonempty_byte_length(input, "FieldName", 1, 63)?;
        for (index, byte) in input.bytes().enumerate() {
            let valid = if index == 0 {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_alphanumeric()
            };
            if !valid {
                return Err(ValidationError::Character {
                    scalar: "FieldName",
                    index,
                });
            }
        }
        Ok(Self(input.to_owned()))
    }

    /// Returns the validated wire representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct XattrName(String);

impl XattrName {
    /// Parses a visible-ASCII extended attribute name.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        validate_nonempty_byte_length(input, "XattrName", 1, 255)?;
        for (index, byte) in input.bytes().enumerate() {
            if !(b'!'..=b'~').contains(&byte) || byte == b'=' {
                return Err(ValidationError::Character {
                    scalar: "XattrName",
                    index,
                });
            }
        }
        Ok(Self(input.to_owned()))
    }

    /// Returns the validated wire representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct LogicalAddress(String);

impl LogicalAddress {
    /// Parses an already-canonical printable-ASCII logical address.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        validate_nonempty_byte_length(input, "LogicalAddress", 1, 255)?;
        let bytes = input.as_bytes();
        for (index, byte) in bytes.iter().copied().enumerate() {
            if !(b' '..=b'~').contains(&byte) {
                return Err(ValidationError::Character {
                    scalar: "LogicalAddress",
                    index,
                });
            }
        }
        if bytes.first() == Some(&b' ') {
            return Err(ValidationError::Character {
                scalar: "LogicalAddress",
                index: 0,
            });
        }
        if bytes.last() == Some(&b' ') {
            return Err(ValidationError::Character {
                scalar: "LogicalAddress",
                index: bytes.len() - 1,
            });
        }
        Ok(Self(input.to_owned()))
    }

    /// Returns the validated wire representation without normalization.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ArtifactName(String);

impl ArtifactName {
    /// Parses an artifact name while preserving its original UTF-8 bytes.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for an empty, overlong, or NUL-containing value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        if input.is_empty() || input.trim().is_empty() {
            return Err(ValidationError::Empty {
                scalar: "ArtifactName",
            });
        }
        let actual = input.chars().count();
        if actual > 255 {
            return Err(ValidationError::Length {
                scalar: "ArtifactName",
                min: 1,
                max: 255,
                actual,
            });
        }
        if let Some(index) = input.find('\0') {
            return Err(ValidationError::Character {
                scalar: "ArtifactName",
                index,
            });
        }
        Ok(Self(input.to_owned()))
    }

    /// Returns the validated, non-normalized UTF-8 value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Parses an opaque, case-sensitive visible-ASCII idempotency key.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        validate_nonempty_byte_length(input, "IdempotencyKey", 16, 128)?;
        for (index, byte) in input.bytes().enumerate() {
            if !(b'!'..=b'~').contains(&byte) {
                return Err(ValidationError::Character {
                    scalar: "IdempotencyKey",
                    index,
                });
            }
        }
        Ok(Self(input.to_owned()))
    }

    /// Returns the validated opaque bytes as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl_parse_deserialize!(
    Digest,
    RawDigest,
    IncarnationId,
    ResourceKey,
    EffectId,
    Hex256,
    Identifier,
    FieldName,
    XattrName,
    LogicalAddress,
    ArtifactName,
    IdempotencyKey,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SafeUInt(u64);

impl SafeUInt {
    pub const MAX: u64 = 9_007_199_254_740_991;

    /// Validates a JSON-safe unsigned integer.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::Range`] when `value` exceeds [`Self::MAX`].
    pub const fn new(value: u64) -> Result<Self, ValidationError> {
        if value > Self::MAX {
            return Err(ValidationError::Range {
                scalar: "SafeUInt",
                value: value as u128,
                max: Self::MAX as u128,
            });
        }
        Ok(Self(value))
    }

    /// Returns the validated integer.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for SafeUInt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct U64Decimal(u64);

impl U64Decimal {
    pub const MAX: u64 = u64::MAX;

    /// Parses a canonical unsigned 64-bit decimal string.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical or out-of-range value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        parse_decimal(input, "U64Decimal").map(Self)
    }

    /// Constructs the canonical decimal value for any `u64`.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Returns the represented integer.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Adds without wrapping.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::Overflow`] if the sum exceeds [`Self::MAX`].
    pub const fn checked_add(self, rhs: u64) -> Result<Self, ValidationError> {
        match self.0.checked_add(rhs) {
            Some(value) => Ok(Self(value)),
            None => Err(ValidationError::Overflow {
                scalar: "U64Decimal",
            }),
        }
    }

    /// Subtracts without wrapping.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::Overflow`] if the difference would be negative.
    pub const fn checked_sub(self, rhs: u64) -> Result<Self, ValidationError> {
        match self.0.checked_sub(rhs) {
            Some(value) => Ok(Self(value)),
            None => Err(ValidationError::Overflow {
                scalar: "U64Decimal",
            }),
        }
    }
}

impl Serialize for U64Decimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for U64Decimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::parse(&input).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ByteLength(U64Decimal);

impl ByteLength {
    /// Parses a canonical unsigned 64-bit decimal byte count.
    ///
    /// # Errors
    ///
    /// Returns the closed validation category for a noncanonical or out-of-range value.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        parse_decimal(input, "ByteLength").map(|value| Self(U64Decimal(value)))
    }

    /// Constructs a byte count for any `u64`.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(U64Decimal::from_u64(value))
    }

    /// Returns the byte count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl<'de> Deserialize<'de> for ByteLength {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::parse(&input).map_err(serde::de::Error::custom)
    }
}

macro_rules! time_scalar {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(U64Decimal);

        impl $name {
            /// Parses a canonical unsigned 64-bit decimal timestamp.
            ///
            /// # Errors
            ///
            /// Returns the closed validation category for a noncanonical or out-of-range value.
            pub fn parse(input: &str) -> Result<Self, ValidationError> {
                parse_decimal(input, stringify!($name)).map(|value| Self(U64Decimal(value)))
            }

            /// Returns the represented timestamp.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0.get()
            }

            /// Adds without wrapping.
            ///
            /// # Errors
            ///
            /// Returns [`ValidationError::Overflow`] if the sum exceeds `u64::MAX`.
            pub const fn checked_add(self, rhs: u64) -> Result<Self, ValidationError> {
                match self.0.get().checked_add(rhs) {
                    Some(value) => Ok(Self(U64Decimal(value))),
                    None => Err(ValidationError::Overflow {
                        scalar: stringify!($name),
                    }),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let input = String::deserialize(deserializer)?;
                Self::parse(&input).map_err(serde::de::Error::custom)
            }
        }
    };
}

time_scalar!(UnixSeconds);
time_scalar!(UnixNanoseconds);

fn validate_digest(input: &str, scalar: &'static str) -> Result<(), ValidationError> {
    if input.is_empty() {
        return Err(ValidationError::Empty { scalar });
    }
    if input.len() != 71 {
        return Err(ValidationError::Length {
            scalar,
            min: 71,
            max: 71,
            actual: input.len(),
        });
    }
    let Some(hex) = input.strip_prefix("sha256:") else {
        return Err(ValidationError::Format { scalar });
    };
    validate_lower_hex(hex, scalar, 64, 7)?;
    if hex.bytes().all(|byte| byte == b'0') {
        return Err(ValidationError::Zero { scalar });
    }
    Ok(())
}

fn validate_lower_hex(
    input: &str,
    scalar: &'static str,
    expected_length: usize,
    index_offset: usize,
) -> Result<(), ValidationError> {
    if input.is_empty() {
        return Err(ValidationError::Empty { scalar });
    }
    if input.len() != expected_length {
        return Err(ValidationError::Length {
            scalar,
            min: expected_length,
            max: expected_length,
            actual: input.len(),
        });
    }
    for (index, byte) in input.bytes().enumerate() {
        if !matches!(byte, b'0'..=b'9' | b'a'..=b'f') {
            return Err(ValidationError::Character {
                scalar,
                index: index + index_offset,
            });
        }
    }
    Ok(())
}

fn validate_nonempty_byte_length(
    input: &str,
    scalar: &'static str,
    min: usize,
    max: usize,
) -> Result<(), ValidationError> {
    if input.is_empty() {
        return Err(ValidationError::Empty { scalar });
    }
    let actual = input.len();
    if !(min..=max).contains(&actual) {
        return Err(ValidationError::Length {
            scalar,
            min,
            max,
            actual,
        });
    }
    Ok(())
}

fn parse_decimal(input: &str, scalar: &'static str) -> Result<u64, ValidationError> {
    if input.is_empty() {
        return Err(ValidationError::Empty { scalar });
    }
    if input != "0" && input.starts_with('0') || !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ValidationError::Format { scalar });
    }
    let value = input
        .parse::<u128>()
        .map_err(|_| ValidationError::Format { scalar })?;
    if value > u128::from(u64::MAX) {
        return Err(ValidationError::Range {
            scalar,
            value,
            max: u128::from(u64::MAX),
        });
    }
    u64::try_from(value).map_err(|_| ValidationError::Range {
        scalar,
        value,
        max: u128::from(u64::MAX),
    })
}
