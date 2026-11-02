//! JSONC preprocessing preserves string contents and rejects duplicate object keys.
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use std::fmt;

pub(crate) fn parse(bytes: &[u8]) -> Result<Value, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "configuration is not UTF-8")?;
    let mut bytes = text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .as_bytes()
        .to_vec();
    let (mut i, mut string, mut escaped) = (0, false, false);
    while i < bytes.len() {
        let byte = bytes[i];
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
            i += 1;
            continue;
        }
        if byte == b'"' {
            string = true;
            i += 1;
            continue;
        }
        if byte == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && !matches!(bytes[i], b'\r' | b'\n') {
                bytes[i] = b' ';
                i += 1;
            }
        } else if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
            bytes[i] = b' ';
            bytes[i + 1] = b' ';
            i += 2;
            let mut closed = false;
            while i < bytes.len() {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    bytes[i] = b' ';
                    bytes[i + 1] = b' ';
                    i += 2;
                    closed = true;
                    break;
                }
                if !matches!(bytes[i], b'\r' | b'\n') {
                    bytes[i] = b' ';
                }
                i += 1;
            }
            if !closed {
                return Err("unterminated block comment".into());
            }
        } else {
            i += 1;
        }
    }
    let (mut string, mut escaped, mut previous) = (false, false, None);
    for i in 0..bytes.len() {
        let byte = bytes[i];
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
        } else if byte == b'"' {
            string = true;
        } else if byte == b',' && previous.is_some_and(|b| !matches!(b, b'{' | b'[' | b',' | b':'))
        {
            let next = bytes[i + 1..].iter().find(|b| !b.is_ascii_whitespace());
            if matches!(next, Some(b'}' | b']')) {
                bytes[i] = b' ';
                continue;
            }
        }
        if !byte.is_ascii_whitespace() {
            previous = Some(byte);
        }
    }
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let result = UniqueValue::deserialize(&mut deserializer).map_err(|e| e.to_string())?;
    deserializer.end().map_err(|e| e.to_string())?;
    Ok(result.0)
}
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded JSON with unique object keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| UniqueValue(Value::Number(n)))
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = access.next_element()? {
                    if values.len() >= 4096 {
                        return Err(de::Error::custom("JSON array exceeds 4096 entries"));
                    }
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = access.next_key::<String>()? {
                    if values.len() >= 4096 {
                        return Err(de::Error::custom("JSON object exceeds 4096 entries"));
                    }
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate JSON object key"));
                    }
                    let UniqueValue(value) = access.next_value()?;
                    values.insert(key, value);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}
