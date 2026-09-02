#![allow(dead_code)]

use guild_effect_kernel::{
    body::{LocalFileObservation, ValidatedBody, validated_body},
    scalar::{Identifier, LogicalAddress, UnixNanoseconds},
};

pub fn absent_observation(address: &str) -> ValidatedBody<LocalFileObservation> {
    validated_body(LocalFileObservation::absent(
        LogicalAddress::parse(address).unwrap(),
        Identifier::parse("host-probe").unwrap(),
        UnixNanoseconds::parse("1788210000000000000").unwrap(),
    ))
    .unwrap()
}
