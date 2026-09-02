use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    fmt,
    sync::Arc,
};

use serde::{
    Serialize,
    de::{self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor},
    ser::{
        self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
};
use serde_json::{Map, Number, Value};
use sha2::{Digest as _, Sha256};

use crate::scalar::{Digest, SafeUInt, ValidationError};

/// Failures while decoding or encoding protocol-canonical JSON.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CanonicalError {
    #[error("duplicate JSON member `{key}`")]
    DuplicateMember { key: String },
    #[error("JSON number is outside the canonical SafeUInt model")]
    Number,
    #[error("JSON decode failed: {0}")]
    Decode(#[source] Arc<serde_json::Error>),
    #[error("JCS encoding failed: {0}")]
    Encode(String),
    #[error("canonical digest was invalid: {0}")]
    Digest(#[from] ValidationError),
}

impl From<serde_json::Error> for CanonicalError {
    fn from(error: serde_json::Error) -> Self {
        Self::Decode(Arc::new(error))
    }
}

impl PartialEq for CanonicalError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::DuplicateMember { key: left }, Self::DuplicateMember { key: right }) => {
                left == right
            }
            (Self::Number, Self::Number) => true,
            (Self::Decode(left), Self::Decode(right)) => {
                left.classify() == right.classify()
                    && left.line() == right.line()
                    && left.column() == right.column()
                    && left.to_string() == right.to_string()
            }
            (Self::Encode(left), Self::Encode(right)) => left == right,
            (Self::Digest(left), Self::Digest(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for CanonicalError {}

/// Serializes a value using RFC 8785 after enforcing the protocol's SafeUInt-only number model.
///
/// # Errors
///
/// Returns [`CanonicalError::Number`] for any negative, floating-point, or oversized number,
/// and [`CanonicalError::Encode`] if the value cannot be serialized.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    let number_rejected = Cell::new(false);
    let result = NumberChecked {
        value,
        number_rejected: &number_rejected,
    }
    .serialize(serde_json::value::Serializer);
    if number_rejected.get() {
        return Err(CanonicalError::Number);
    }
    let value = result.map_err(|error| CanonicalError::Encode(error.to_string()))?;
    encode_value(&value)
}

fn encode_value(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    match value {
        Value::Array(values) => {
            let mut output = vec![b'['];
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                output.extend(encode_value(value)?);
            }
            output.push(b']');
            Ok(output)
        }
        Value::Object(object) => {
            let mut members: Vec<_> = object.iter().collect();
            members.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));

            let mut output = vec![b'{'];
            for (index, (name, value)) in members.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                output.extend(
                    serde_jcs::to_vec(name)
                        .map_err(|error| CanonicalError::Encode(error.to_string()))?,
                );
                output.push(b':');
                output.extend(encode_value(value)?);
            }
            output.push(b'}');
            Ok(output)
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            serde_jcs::to_vec(value).map_err(|error| CanonicalError::Encode(error.to_string()))
        }
    }
}

/// Hashes exactly the RFC 8785 bytes of a value with SHA-256.
///
/// # Errors
///
/// Returns the error from canonical serialization or digest validation.
pub fn canonical_digest<T: Serialize>(value: &T) -> Result<Digest, CanonicalError> {
    let bytes = canonical_bytes(value)?;
    let hash = Sha256::digest(bytes);
    Digest::parse(&format!("sha256:{}", hex::encode(hash))).map_err(CanonicalError::from)
}

/// Strictly decodes one JSON value after recursively rejecting duplicate members and numbers
/// outside the protocol's `SafeUInt` lexical model.
///
/// # Errors
///
/// Returns a closed canonicalization error for duplicate members, inadmissible numbers, trailing
/// bytes, malformed JSON, or failure to deserialize the validated value as `T`.
pub fn strict_from_slice<T: DeserializeOwned>(input: &[u8]) -> Result<T, CanonicalError> {
    let issue = RefCell::new(None);
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let value = match (StrictValueSeed { issue: &issue }).deserialize(&mut deserializer) {
        Ok(value) => value,
        Err(error) => return Err(issue_to_error(&issue).unwrap_or_else(|| error.into())),
    };
    deserializer.end().map_err(CanonicalError::from)?;
    T::deserialize(value).map_err(CanonicalError::from)
}

#[derive(Debug)]
enum StrictIssue {
    DuplicateMember(String),
    Number,
}

fn issue_to_error(issue: &RefCell<Option<StrictIssue>>) -> Option<CanonicalError> {
    issue.borrow_mut().take().map(|issue| match issue {
        StrictIssue::DuplicateMember(key) => CanonicalError::DuplicateMember { key },
        StrictIssue::Number => CanonicalError::Number,
    })
}

#[derive(Clone, Copy)]
struct StrictValueSeed<'a> {
    issue: &'a RefCell<Option<StrictIssue>>,
}

impl StrictValueSeed<'_> {
    fn reject<E: de::Error>(self, issue: StrictIssue) -> E {
        if self.issue.borrow().is_none() {
            self.issue.replace(Some(issue));
        }
        E::custom("strict canonical JSON rejection")
    }
}

impl<'de> DeserializeSeed<'de> for StrictValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for StrictValueSeed<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value in the canonical SafeUInt model")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        let value = u64::try_from(value).map_err(|_| self.reject(StrictIssue::Number))?;
        self.visit_u64(value)
    }

    fn visit_i128<E: de::Error>(self, value: i128) -> Result<Self::Value, E> {
        let value = u64::try_from(value).map_err(|_| self.reject(StrictIssue::Number))?;
        self.visit_u64(value)
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        if value > SafeUInt::MAX {
            return Err(self.reject(StrictIssue::Number));
        }
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u128<E: de::Error>(self, value: u128) -> Result<Self::Value, E> {
        let value = u64::try_from(value).map_err(|_| self.reject(StrictIssue::Number))?;
        self.visit_u64(value)
    }

    fn visit_f64<E: de::Error>(self, _value: f64) -> Result<Self::Value, E> {
        Err(self.reject(StrictIssue::Number))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_borrowed_str<E: de::Error>(self, value: &'de str) -> Result<Self::Value, E> {
        self.visit_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        self.deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(self)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut names = BTreeSet::new();
        let mut members = Vec::new();
        while let Some(name) = object.next_key::<String>()? {
            if !names.insert(name.clone()) {
                return Err(self.reject(StrictIssue::DuplicateMember(name)));
            }
            let value = object.next_value_seed(self)?;
            members.push((name, value));
        }

        let mut object = Map::new();
        for (name, value) in members {
            object.insert(name, value);
        }
        Ok(Value::Object(object))
    }
}

struct NumberChecked<'a, T: ?Sized> {
    value: &'a T,
    number_rejected: &'a Cell<bool>,
}

impl<T: ?Sized + Serialize> Serialize for NumberChecked<'_, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.value.serialize(NumberCheckingSerializer {
            inner: serializer,
            number_rejected: self.number_rejected,
        })
    }
}

struct NumberCheckingSerializer<'a, S> {
    inner: S,
    number_rejected: &'a Cell<bool>,
}

impl<'a, S: ser::Serializer> NumberCheckingSerializer<'a, S> {
    fn reject_number<T>(self) -> Result<T, S::Error> {
        self.number_rejected.set(true);
        Err(ser::Error::custom(
            "number is outside the canonical SafeUInt model",
        ))
    }

    fn checked<T: ?Sized + Serialize>(&self, value: &'a T) -> NumberChecked<'a, T> {
        NumberChecked {
            value,
            number_rejected: self.number_rejected,
        }
    }
}

impl<'a, S: ser::Serializer> ser::Serializer for NumberCheckingSerializer<'a, S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = NumberCheckingCompound<'a, S::SerializeSeq>;
    type SerializeTuple = NumberCheckingCompound<'a, S::SerializeTuple>;
    type SerializeTupleStruct = NumberCheckingCompound<'a, S::SerializeTupleStruct>;
    type SerializeTupleVariant = NumberCheckingCompound<'a, S::SerializeTupleVariant>;
    type SerializeMap = NumberCheckingCompound<'a, S::SerializeMap>;
    type SerializeStruct = NumberCheckingCompound<'a, S::SerializeStruct>;
    type SerializeStructVariant = NumberCheckingCompound<'a, S::SerializeStructVariant>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bool(value)
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        if value < 0 {
            return self.reject_number();
        }
        self.inner.serialize_i8(value)
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        if value < 0 {
            return self.reject_number();
        }
        self.inner.serialize_i16(value)
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        if value < 0 {
            return self.reject_number();
        }
        self.inner.serialize_i32(value)
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        let Ok(unsigned) = u64::try_from(value) else {
            return self.reject_number();
        };
        if unsigned > SafeUInt::MAX {
            return self.reject_number();
        }
        self.inner.serialize_i64(value)
    }

    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        if !(0..=i128::from(SafeUInt::MAX)).contains(&value) {
            return self.reject_number();
        }
        self.inner.serialize_i128(value)
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u8(value)
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u16(value)
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u32(value)
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        if value > SafeUInt::MAX {
            return self.reject_number();
        }
        self.inner.serialize_u64(value)
    }

    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        if value > u128::from(SafeUInt::MAX) {
            return self.reject_number();
        }
        self.inner.serialize_u128(value)
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        self.reject_number()
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        self.reject_number()
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_char(value)
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_str(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        let checked = self.checked(value);
        self.inner.serialize_some(&checked)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner
            .serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        let checked = self.checked(value);
        self.inner.serialize_newtype_struct(name, &checked)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        let checked = self.checked(value);
        self.inner
            .serialize_newtype_variant(name, variant_index, variant, &checked)
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(NumberCheckingCompound {
            inner: self.inner.serialize_seq(length)?,
            number_rejected: self.number_rejected,
        })
    }

    fn serialize_tuple(self, length: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(NumberCheckingCompound {
            inner: self.inner.serialize_tuple(length)?,
            number_rejected: self.number_rejected,
        })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(NumberCheckingCompound {
            inner: self.inner.serialize_tuple_struct(name, length)?,
            number_rejected: self.number_rejected,
        })
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(NumberCheckingCompound {
            inner: self
                .inner
                .serialize_tuple_variant(name, variant_index, variant, length)?,
            number_rejected: self.number_rejected,
        })
    }

    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(NumberCheckingCompound {
            inner: self.inner.serialize_map(length)?,
            number_rejected: self.number_rejected,
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(NumberCheckingCompound {
            inner: self.inner.serialize_struct(name, length)?,
            number_rejected: self.number_rejected,
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(NumberCheckingCompound {
            inner: self
                .inner
                .serialize_struct_variant(name, variant_index, variant, length)?,
            number_rejected: self.number_rejected,
        })
    }

    fn collect_str<T: ?Sized + fmt::Display>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.inner.collect_str(value)
    }

    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

struct NumberCheckingCompound<'a, C> {
    inner: C,
    number_rejected: &'a Cell<bool>,
}

impl<C: SerializeSeq> SerializeSeq for NumberCheckingCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_element(&NumberChecked {
            value,
            number_rejected: self.number_rejected,
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTuple> SerializeTuple for NumberCheckingCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_element(&NumberChecked {
            value,
            number_rejected: self.number_rejected,
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTupleStruct> SerializeTupleStruct for NumberCheckingCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(&NumberChecked {
            value,
            number_rejected: self.number_rejected,
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTupleVariant> SerializeTupleVariant for NumberCheckingCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(&NumberChecked {
            value,
            number_rejected: self.number_rejected,
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeMap> SerializeMap for NumberCheckingCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.inner.serialize_key(&NumberChecked {
            value: key,
            number_rejected: self.number_rejected,
        })
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_value(&NumberChecked {
            value,
            number_rejected: self.number_rejected,
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeStruct> SerializeStruct for NumberCheckingCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.inner.serialize_field(
            key,
            &NumberChecked {
                value,
                number_rejected: self.number_rejected,
            },
        )
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeStructVariant> SerializeStructVariant for NumberCheckingCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.inner.serialize_field(
            key,
            &NumberChecked {
                value,
                number_rejected: self.number_rejected,
            },
        )
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}
