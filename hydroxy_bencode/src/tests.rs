#![allow(unused_imports)]
use crate::decode::decode;
use crate::{decode::BencodedValue, encode::encode};
use std::collections::BTreeMap;

#[test]
fn positive_integer_decode_test() {
    let enc_val = b"i98e";
    let result = decode(enc_val).unwrap();
    assert_eq!(result, BencodedValue::Integer(98));
}

#[test]
fn positive_integer_decode_test_2() {
    let enc_val = b"l4:spam4:eggse";
    let result = decode(enc_val).unwrap();
    assert_eq!(
        result,
        BencodedValue::List(vec![
            BencodedValue::String(b"spam".to_vec()),
            BencodedValue::String(b"eggs".to_vec())
        ])
    );
}

#[test]
fn negative_integer_decode_test() {
    let enc_val = b"i-746e";
    let result = decode(enc_val).unwrap();

    assert_eq!(result, BencodedValue::Integer(-746));
}

#[test]
fn test_string() {
    let input = b"4:spam";

    let result = decode(input).unwrap();
    assert_eq!(result, BencodedValue::String(b"spam".to_vec()));
}

#[test]
fn test_encode_integer() {
    let value = BencodedValue::Integer(123);
    let encoded = encode(&value);
    assert_eq!(encoded, b"i123e");
}

#[test]
fn test_encode_string() {
    let value = BencodedValue::String(b"hello".to_vec());
    let encoded = encode(&value);
    assert_eq!(encoded, b"5:hello");
}

#[test]
fn test_encode_list() {
    let value = BencodedValue::List(vec![
        BencodedValue::String(b"spam".to_vec()),
        BencodedValue::String(b"eggs".to_vec()),
    ]);
    let encoded = encode(&value);
    assert_eq!(encoded, b"l4:spam4:eggse");
}

#[test]
fn test_encode_dictionary() {
    let mut dict = BTreeMap::new();
    dict.insert(b"cow".to_vec(), BencodedValue::String(b"moo".to_vec()));
    dict.insert(b"spam".to_vec(), BencodedValue::String(b"eggs".to_vec()));
    let value = BencodedValue::Dictionary(dict);
    let encoded = encode(&value);
    // Keys must be sorted alphabetically: "cow" then "spam"
    assert_eq!(encoded, b"d3:cow3:moo4:spam4:eggse");
}

#[test]
fn test_encode_decode_roundtrip() {
    let mut dict = BTreeMap::new();
    dict.insert(b"key".to_vec(), BencodedValue::String(b"value".to_vec()));
    let original_value = BencodedValue::List(vec![
        BencodedValue::Integer(-42),
        BencodedValue::String(b"hello world".to_vec()),
        BencodedValue::Dictionary(dict),
    ]);
    let encoded = encode(&original_value);
    let decoded = decode(&encoded).unwrap();
    assert_eq!(original_value, decoded);
}
