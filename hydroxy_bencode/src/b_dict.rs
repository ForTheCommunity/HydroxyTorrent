use std::collections::BTreeMap;

use crate::decode::BencodedValue;

pub struct BDict<'a> {
    pub inner: &'a BTreeMap<Vec<u8>, BencodedValue>,
}

impl<'a> BDict<'a> {
    pub fn new(inner: &'a BTreeMap<Vec<u8>, BencodedValue>) -> Self {
        Self { inner }
    }

    pub fn get_string(&self, key: &[u8]) -> Option<&[u8]> {
        match self.inner.get(key)? {
            BencodedValue::String(v) => Some(v),
            _ => None,
        }
    }

    pub fn get_int(&self, key: &[u8]) -> Option<i64> {
        match self.inner.get(key)? {
            BencodedValue::Integer(v) => Some(*v),
            _ => None,
        }
    }

    pub fn get_list(&self, key: &[u8]) -> Option<&Vec<BencodedValue>> {
        match self.inner.get(key)? {
            BencodedValue::List(v) => Some(v),
            _ => None,
        }
    }

    pub fn get_dict(&self, key: &[u8]) -> Option<BDict<'_>> {
        match self.inner.get(key)? {
            BencodedValue::Dictionary(v) => Some(BDict::new(v)),
            _ => None,
        }
    }
}
