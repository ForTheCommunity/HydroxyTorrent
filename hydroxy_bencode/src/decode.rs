use std::collections::BTreeMap;
use std::fmt;

use crate::errors::DecodeError;

#[derive(PartialEq, PartialOrd)]
pub enum BencodedValue {
    String(Vec<u8>),
    Integer(i64),
    List(Vec<BencodedValue>),
    Dictionary(BTreeMap<Vec<u8>, BencodedValue>),
}

impl fmt::Debug for BencodedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BencodedValue::String(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => write!(f, "{:?}", s),
                Err(_) => write!(f, "{:?}", bytes),
            },
            BencodedValue::Integer(i) => write!(f, "{}", i),
            BencodedValue::List(list) => f.debug_list().entries(list).finish(),
            BencodedValue::Dictionary(dict) => {
                let mut map = f.debug_map();
                for (k, v) in dict {
                    match std::str::from_utf8(k) {
                        Ok(s) => map.entry(&s, v),
                        Err(_) => map.entry(k, v),
                    };
                }
                map.finish()
            }
        }
    }
}

pub fn decode(encoded_data: &[u8]) -> Result<BencodedValue, DecodeError> {
    let (value, _remainder) = decode_element(encoded_data)?;
    Ok(value)
}

fn decode_element(encoded_data: &[u8]) -> Result<(BencodedValue, &[u8]), DecodeError> {
    if encoded_data.is_empty() {
        return Err(DecodeError::Empty);
    }

    match encoded_data[0] {
        b'i' => decode_integer(encoded_data),
        b'0'..=b'9' => decode_string(encoded_data),
        b'l' => decode_list(encoded_data),
        b'd' => decode_dictionary(encoded_data),
        _ => Err(DecodeError::InvalidBytes),
    }
}

fn decode_integer(encoded_data: &[u8]) -> Result<(BencodedValue, &[u8]), DecodeError> {
    if let Some(end_index) = encoded_data.iter().position(|&b| b == b'e') {
        let integer_slice = &encoded_data[1..end_index];
        let integer_str =
            std::str::from_utf8(integer_slice).map_err(|_e| DecodeError::InvalidInteger)?;
        let integer = integer_str
            .parse::<i64>()
            .map_err(|_e| DecodeError::InvalidInteger)?;
        Ok((
            BencodedValue::Integer(integer),
            &encoded_data[end_index + 1..],
        ))
    } else {
        Err(DecodeError::InvalidInteger)
    }
}

fn decode_string(encoded_data: &[u8]) -> Result<(BencodedValue, &[u8]), DecodeError> {
    // position of colon ':'
    if let Some(colon_index) = encoded_data.iter().position(|&b| b == b':') {
        // finding string length in byte slice
        let string_length_slice: &[u8] = &encoded_data[..colon_index];
        // converting bytes to string slice
        let length_str: &str =
            std::str::from_utf8(string_length_slice).map_err(|_e| DecodeError::InvalidString)?;

        // length of string in Number (i64)
        let length_of_number: usize = length_str
            .parse::<usize>()
            .map_err(|_e| DecodeError::InvalidString)?;

        // start of string
        let start_of_string = colon_index + 1;

        // end of string
        let end_of_string = length_of_number + start_of_string;

        if end_of_string > encoded_data.len() {
            return Err(DecodeError::InvalidString);
        }

        let string_data: Vec<u8> = encoded_data[start_of_string..end_of_string].to_vec();
        // remaining slice after string data
        let remaining = &encoded_data[end_of_string..];

        Ok((BencodedValue::String(string_data), remaining))
    } else {
        Err(DecodeError::InvalidString)
    }
}

fn decode_list(encoded_data: &[u8]) -> Result<(BencodedValue, &[u8]), DecodeError> {
    let mut rest = &encoded_data[1..];
    let mut list = Vec::new();

    while !rest.is_empty() && rest[0] != b'e' {
        let (value, remainder) = decode_element(rest)?;
        list.push(value);
        rest = remainder;
    }

    if rest.is_empty() {
        return Err(DecodeError::InvalidList);
    }

    Ok((BencodedValue::List(list), &rest[1..]))
}

fn decode_dictionary(encoded_data: &[u8]) -> Result<(BencodedValue, &[u8]), DecodeError> {
    let mut rest = &encoded_data[1..];
    let mut dict = BTreeMap::new();

    while !rest.is_empty() && rest[0] != b'e' {
        let (key, remainder) = decode_element(rest)?;
        let key = match key {
            BencodedValue::String(k) => k,
            _ => return Err(DecodeError::InvalidDictionary),
        };

        let (value, remainder) = decode_element(remainder)?;
        dict.insert(key, value);
        rest = remainder;
    }

    if rest.is_empty() {
        return Err(DecodeError::InvalidDictionary);
    }

    Ok((BencodedValue::Dictionary(dict), &rest[1..]))
}
