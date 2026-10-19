//! Canonical CBOR (RFC 8949 §4.2.1) for signing, hashing, and the node wire format.
//!
//! The encoder uses the shortest integer form, definite lengths only, and map
//! keys sorted by their encoded bytes. Floats, tags, and indefinite lengths are
//! not representable. The decoder rejects anything that is not already in that
//! form, including a map whose keys are not strictly ascending.
//!
//! This is not the `.acpbackup` header. That file is format version 1: a definite
//! CBOR array used as AEAD additional data. It stays as it is.

use anyhow::{Result, bail};
use std::cmp::Ordering;

/// One canonical CBOR value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cbor {
    /// Unsigned integer.
    Uint(u64),
    /// Negative integer, stored as the CBOR argument `n` in `-1 - n`.
    Neg(u64),
    /// Byte string.
    Bytes(Vec<u8>),
    /// UTF-8 text.
    Text(String),
    /// Definite array, in order.
    Array(Vec<Cbor>),
    /// Map. Encoding sorts keys; the input order does not matter.
    Map(Vec<(Cbor, Cbor)>),
    /// `false` or `true`.
    Bool(bool),
    /// `null`.
    Null,
}

impl Cbor {
    /// Text string.
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// Byte string.
    pub fn bytes(b: impl AsRef<[u8]>) -> Self {
        Self::Bytes(b.as_ref().to_vec())
    }

    /// Unsigned integer.
    pub fn uint(n: u64) -> Self {
        Self::Uint(n)
    }

    /// Signed integer. `i64::MIN` is `-1 - (i64::MAX as u64 + 1)`.
    pub fn int(n: i64) -> Self {
        if n >= 0 {
            Self::Uint(n as u64)
        } else {
            Self::Neg((n as u64).wrapping_neg() - 1)
        }
    }

    /// Definite array.
    pub fn array(items: Vec<Cbor>) -> Self {
        Self::Array(items)
    }

    /// Map. Duplicate keys are an error at encode time.
    pub fn map(entries: Vec<(Cbor, Cbor)>) -> Self {
        Self::Map(entries)
    }
}

/// Encode `value` in canonical form.
pub fn to_vec(value: &Cbor) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    encode(value, &mut out)?;
    Ok(out)
}

/// `blake3` of the canonical encoding. This is `record_hash` for a record.
pub fn blake3_hash(value: &Cbor) -> Result<[u8; 32]> {
    Ok(*blake3::hash(&to_vec(value)?).as_bytes())
}

/// Decode one canonical value. Trailing bytes are an error.
pub fn from_slice(input: &[u8]) -> Result<Cbor> {
    let (value, rest) = decode(input)?;
    if !rest.is_empty() {
        bail!("canonical CBOR has trailing bytes");
    }
    Ok(value)
}

fn encode(value: &Cbor, out: &mut Vec<u8>) -> Result<()> {
    match value {
        Cbor::Uint(n) => write_type(out, 0, *n),
        Cbor::Neg(n) => write_type(out, 1, *n),
        Cbor::Bytes(b) => {
            write_type(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Cbor::Text(s) => {
            write_type(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Cbor::Array(items) => {
            write_type(out, 4, items.len() as u64);
            for item in items {
                encode(item, out)?;
            }
        }
        Cbor::Map(entries) => {
            let mut keyed = Vec::with_capacity(entries.len());
            for (key, val) in entries {
                let mut key_bytes = Vec::new();
                encode(key, &mut key_bytes)?;
                keyed.push((key_bytes, key, val));
            }
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            for pair in keyed.windows(2) {
                if pair[0].0 == pair[1].0 {
                    bail!("canonical CBOR map has a duplicate key");
                }
            }
            write_type(out, 5, keyed.len() as u64);
            for (key_bytes, _, val) in &keyed {
                out.extend_from_slice(key_bytes);
                encode(val, out)?;
            }
        }
        Cbor::Bool(false) => out.push(0xf4),
        Cbor::Bool(true) => out.push(0xf5),
        Cbor::Null => out.push(0xf6),
    }
    Ok(())
}

fn write_type(out: &mut Vec<u8>, major: u8, n: u64) {
    let maj = major << 5;
    if n < 24 {
        out.push(maj | n as u8);
    } else if n <= u8::MAX as u64 {
        out.push(maj | 24);
        out.push(n as u8);
    } else if n <= u16::MAX as u64 {
        out.push(maj | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= u32::MAX as u64 {
        out.push(maj | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(maj | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn decode(input: &[u8]) -> Result<(Cbor, &[u8])> {
    if input.is_empty() {
        bail!("canonical CBOR ended early");
    }
    let head = input[0];
    let major = head >> 5;
    let ai = head & 0x1f;
    if major == 6 {
        bail!("canonical CBOR rejects tags");
    }
    if major == 7 {
        return decode_simple(ai, &input[1..]);
    }
    if ai == 31 {
        bail!("canonical CBOR rejects indefinite lengths");
    }
    let (n, rest) = read_len(ai, &input[1..])?;
    match major {
        0 => Ok((Cbor::Uint(n), rest)),
        1 => Ok((Cbor::Neg(n), rest)),
        2 => {
            let (bytes, rest) = take_len(rest, n, "byte string")?;
            Ok((Cbor::Bytes(bytes.to_vec()), rest))
        }
        3 => {
            let (bytes, rest) = take_len(rest, n, "text string")?;
            let text = std::str::from_utf8(bytes).map_err(|_| anyhow::anyhow!("CBOR text is not UTF-8"))?;
            Ok((Cbor::Text(text.to_string()), rest))
        }
        4 => {
            let mut rest = rest;
            let mut items = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let (item, next) = decode(rest)?;
                items.push(item);
                rest = next;
            }
            Ok((Cbor::Array(items), rest))
        }
        5 => decode_map(n, rest),
        _ => bail!("canonical CBOR has an unknown major type"),
    }
}

fn decode_simple(ai: u8, rest: &[u8]) -> Result<(Cbor, &[u8])> {
    match ai {
        20 => Ok((Cbor::Bool(false), rest)),
        21 => Ok((Cbor::Bool(true), rest)),
        22 => Ok((Cbor::Null, rest)),
        25..=27 => bail!("canonical CBOR rejects floats"),
        31 => bail!("canonical CBOR rejects indefinite lengths"),
        _ => bail!("canonical CBOR rejects this simple value"),
    }
}

fn read_len(ai: u8, rest: &[u8]) -> Result<(u64, &[u8])> {
    if ai < 24 {
        return Ok((u64::from(ai), rest));
    }
    let (width, min) = match ai {
        24 => (1usize, 24u64),
        25 => (2, 256),
        26 => (4, 65536),
        27 => (8, 65536u64 + 1),
        _ => bail!("canonical CBOR has a bad additional info"),
    };
    if rest.len() < width {
        bail!("canonical CBOR ended early");
    }
    let mut buf = [0u8; 8];
    buf[8 - width..].copy_from_slice(&rest[..width]);
    let n = u64::from_be_bytes(buf);
    if ai == 27 {
        if n <= u32::MAX as u64 {
            bail!("canonical CBOR integer is not in shortest form");
        }
    } else if n < min {
        bail!("canonical CBOR integer is not in shortest form");
    }
    Ok((n, &rest[width..]))
}

fn take_len<'a>(rest: &'a [u8], n: u64, what: &str) -> Result<(&'a [u8], &'a [u8])> {
    let n = usize::try_from(n).map_err(|_| anyhow::anyhow!("canonical CBOR {what} is too long"))?;
    if rest.len() < n {
        bail!("canonical CBOR {what} ended early");
    }
    Ok(rest.split_at(n))
}

fn decode_map(n: u64, mut rest: &[u8]) -> Result<(Cbor, &[u8])> {
    let mut entries = Vec::with_capacity(n as usize);
    let mut prev: Option<Vec<u8>> = None;
    for _ in 0..n {
        let start = rest;
        let (key, after_key) = decode(rest)?;
        let key_len = start.len() - after_key.len();
        let key_bytes = start[..key_len].to_vec();
        if let Some(prev) = &prev {
            match key_bytes.cmp(prev) {
                Ordering::Greater => {}
                Ordering::Equal => bail!("canonical CBOR map has a duplicate key"),
                Ordering::Less => bail!("canonical CBOR map keys are not sorted"),
            }
        }
        prev = Some(key_bytes);
        let (val, next) = decode(after_key)?;
        entries.push((key, val));
        rest = next;
    }
    Ok((Cbor::Map(entries), rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(value: Cbor) -> Vec<u8> {
        to_vec(&value).unwrap()
    }

    #[test]
    fn shortest_integers_and_simple_values() {
        assert_eq!(enc(Cbor::uint(0)), vec![0x00]);
        assert_eq!(enc(Cbor::uint(1)), vec![0x01]);
        assert_eq!(enc(Cbor::uint(23)), vec![0x17]);
        assert_eq!(enc(Cbor::uint(24)), vec![0x18, 0x18]);
        assert_eq!(enc(Cbor::uint(255)), vec![0x18, 0xff]);
        assert_eq!(enc(Cbor::uint(256)), vec![0x19, 0x01, 0x00]);
        assert_eq!(enc(Cbor::int(-1)), vec![0x20]);
        assert_eq!(enc(Cbor::int(-24)), vec![0x37]);
        assert_eq!(enc(Cbor::int(-25)), vec![0x38, 0x18]);
        assert_eq!(enc(Cbor::Bool(false)), vec![0xf4]);
        assert_eq!(enc(Cbor::Bool(true)), vec![0xf5]);
        assert_eq!(enc(Cbor::Null), vec![0xf6]);
        assert_eq!(enc(Cbor::bytes([])), vec![0x40]);
        assert_eq!(enc(Cbor::text("")), vec![0x60]);
        assert_eq!(enc(Cbor::array(vec![])), vec![0x80]);
        assert_eq!(enc(Cbor::map(vec![])), vec![0xa0]);
        assert_eq!(enc(Cbor::text("a")), vec![0x61, 0x61]);
        assert_eq!(enc(Cbor::bytes([1, 2])), vec![0x42, 0x01, 0x02]);
        assert_eq!(
            enc(Cbor::array(vec![Cbor::uint(1), Cbor::uint(2)])),
            vec![0x82, 0x01, 0x02]
        );
    }

    #[test]
    fn map_keys_sort_by_encoded_bytes() {
        let map = Cbor::map(vec![
            (Cbor::text("b"), Cbor::uint(1)),
            (Cbor::text("a"), Cbor::uint(2)),
        ]);
        // "a" is 0x6161, "b" is 0x6162, so "a" comes first.
        assert_eq!(
            enc(map),
            vec![0xa2, 0x61, 0x61, 0x02, 0x61, 0x62, 0x01]
        );
        let longer = Cbor::map(vec![
            (Cbor::text("aa"), Cbor::uint(1)),
            (Cbor::text("b"), Cbor::uint(2)),
        ]);
        // "b" encodes as 0x61… and "aa" as 0x62…, so "b" is first.
        assert_eq!(
            enc(longer),
            vec![0xa2, 0x61, 0x62, 0x02, 0x62, 0x61, 0x61, 0x01]
        );
        let nums = Cbor::map(vec![
            (Cbor::uint(10), Cbor::uint(1)),
            (Cbor::uint(2), Cbor::uint(3)),
        ]);
        assert_eq!(enc(nums), vec![0xa2, 0x02, 0x03, 0x0a, 0x01]);
    }

    #[test]
    fn duplicate_keys_are_refused() {
        let map = Cbor::map(vec![
            (Cbor::text("a"), Cbor::uint(1)),
            (Cbor::text("a"), Cbor::uint(2)),
        ]);
        assert!(to_vec(&map).is_err());
    }

    #[test]
    fn round_trip_and_hash_vector() {
        let value = Cbor::map(vec![
            (Cbor::text("type"), Cbor::text("vector")),
            (Cbor::text("n"), Cbor::uint(24)),
        ]);
        let bytes = to_vec(&value).unwrap();
        assert_eq!(
            bytes,
            vec![
                0xa2, 0x61, 0x6e, 0x18, 0x18, 0x64, 0x74, 0x79, 0x70, 0x65, 0x66, 0x76, 0x65, 0x63,
                0x74, 0x6f, 0x72
            ]
        );
        assert_eq!(to_vec(&from_slice(&bytes).unwrap()).unwrap(), bytes);
        let hash = blake3_hash(&value).unwrap();
        assert_eq!(
            data_encoding::HEXLOWER.encode(&hash),
            "f1285198df182da88c543a25dbb058a77c5b5887d10e257157141866e9cede8b"
        );
    }

    #[test]
    fn decoder_rejects_non_canonical_forms() {
        assert!(from_slice(&[0x18, 0x01]).is_err(), "non-shortest int");
        assert!(from_slice(&[0x5f, 0xff]).is_err(), "indefinite bytes");
        assert!(from_slice(&[0xc0, 0x00]).is_err(), "tag");
        assert!(from_slice(&[0xfa, 0, 0, 0, 0]).is_err(), "float");
        // {"b": 1, "a": 2} with keys in the wrong order.
        assert!(from_slice(&[0xa2, 0x61, 0x62, 0x01, 0x61, 0x61, 0x02]).is_err());
        assert!(from_slice(&[0x01, 0x02]).is_err(), "trailing");
    }
}
