// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Ported from https://github.com/foundationdb-rs/foundationdb-rs/blob/main/foundationdb/src/tuple/pack.rs

use crate::Key;
use crate::KeyPart;

//const NIL: u8 = 0x00;
const BYTES: u8 = 0x01;
const STRING: u8 = 0x02;
//const NESTED: u8 = 0x05;
const NEGINTSTART: u8 = 0x0b;
const INTZERO: u8 = 0x14;
const POSINTEND: u8 = 0x1d;
//const FLOAT: u8 = 0x20;
const DOUBLE: u8 = 0x21;
const FALSE: u8 = 0x26;
const TRUE: u8 = 0x27;

const ESCAPE: u8 = 0xff;

const CANONICAL_NAN_POS: u64 = 0x7ff8000000000000u64;
const CANONICAL_NAN_NEG: u64 = 0xfff8000000000000u64;

pub fn canonicalize_f64(n: f64) -> f64 {
  if n.is_nan() {
    if n.is_sign_negative() {
      f64::from_bits(CANONICAL_NAN_NEG)
    } else {
      f64::from_bits(CANONICAL_NAN_POS)
    }
  } else {
    n
  }
}

pub fn encode_key(key: &Key) -> std::io::Result<Vec<u8>> {
  let mut output: Vec<u8> = vec![];
  for part in &key.0 {
    match part {
      KeyPart::String(key) => {
        output.push(STRING);
        escape_raw_bytes_into(&mut output, key.as_bytes());
        output.push(0);
      }
      KeyPart::Int(key) => {
        bigint::encode_into(&mut output, key)?;
      }
      KeyPart::Float(key) => {
        double::encode_into(&mut output, *key);
      }
      KeyPart::Bytes(key) => {
        output.push(BYTES);
        escape_raw_bytes_into(&mut output, key);
        output.push(0);
      }
      KeyPart::False => {
        output.push(FALSE);
      }
      KeyPart::True => {
        output.push(TRUE);
      }
    }
  }
  Ok(output)
}

pub fn decode_key(mut bytes: &[u8]) -> std::io::Result<Key> {
  let mut key = Key(vec![]);
  while !bytes.is_empty() {
    let tag = bytes[0];
    bytes = &bytes[1..];

    let next_bytes = match tag {
      self::STRING => {
        let (next_bytes, data) = parse_slice(bytes)?;
        let data = String::from_utf8(data).map_err(|_| {
          std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid utf8")
        })?;
        key.0.push(KeyPart::String(data));
        next_bytes
      }
      self::NEGINTSTART..=self::POSINTEND => {
        let (next_bytes, data) = bigint::decode_from(bytes, tag)?;
        key.0.push(KeyPart::Int(data));
        next_bytes
      }
      self::DOUBLE => {
        let (next_bytes, data) = double::decode_from(bytes)?;
        key.0.push(KeyPart::Float(data));
        next_bytes
      }
      self::BYTES => {
        let (next_bytes, data) = parse_slice(bytes)?;
        key.0.push(KeyPart::Bytes(data));
        next_bytes
      }
      self::FALSE => {
        key.0.push(KeyPart::False);
        bytes
      }
      self::TRUE => {
        key.0.push(KeyPart::True);
        bytes
      }
      _ => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          "invalid tag",
        ))
      }
    };

    bytes = next_bytes;
  }
  Ok(key)
}

fn escape_raw_bytes_into(out: &mut Vec<u8>, x: &[u8]) {
  for &b in x {
    out.push(b);
    if b == 0 {
      out.push(ESCAPE);
    }
  }
}

mod bigint {
  use num_bigint::BigInt;
  use num_bigint::Sign;

  use super::parse_byte;
  use super::parse_bytes;
  const MAX_SZ: usize = 8;

  // Ported from https://github.com/foundationdb-rs/foundationdb-rs/blob/7415e116d5d96c2630976058de28e439eed7e809/foundationdb/src/tuple/pack.rs#L575
  pub fn encode_into(out: &mut Vec<u8>, key: &BigInt) -> std::io::Result<()> {
    if key.sign() == Sign::NoSign {
      out.push(super::INTZERO);
      return Ok(());
    }
    let (sign, mut bytes) = key.to_bytes_be();
    let n = bytes.len();
    match sign {
      Sign::Minus => {
        if n <= MAX_SZ {
          out.push(super::INTZERO - n as u8);
        } else {
          out.extend_from_slice(&[super::NEGINTSTART, bigint_n(n)? ^ 0xff]);
        }
        invert(&mut bytes);
        out.extend_from_slice(&bytes);
      }
      Sign::NoSign => unreachable!(),
      Sign::Plus => {
        if n <= MAX_SZ {
          out.push(super::INTZERO + n as u8);
        } else {
          out.extend_from_slice(&[super::POSINTEND, bigint_n(n)?]);
        }
        out.extend_from_slice(&bytes);
      }
    }
    Ok(())
  }

  pub fn decode_from(
    input: &[u8],
    tag: u8,
  ) -> std::io::Result<(&[u8], BigInt)> {
    if super::INTZERO <= tag && tag <= super::INTZERO + MAX_SZ as u8 {
      let n = (tag - super::INTZERO) as usize;
      let (input, bytes) = parse_bytes(input, n)?;
      Ok((input, BigInt::from_bytes_be(Sign::Plus, bytes)))
    } else if super::INTZERO - MAX_SZ as u8 <= tag && tag < super::INTZERO {
      let n = (super::INTZERO - tag) as usize;
      let (input, bytes) = parse_bytes(input, n)?;
      Ok((input, BigInt::from_bytes_be(Sign::Minus, &inverted(bytes))))
    } else if tag == super::NEGINTSTART {
      let (input, raw_length) = parse_byte(input)?;
      let n = usize::from(raw_length ^ 0xff);
      let (input, bytes) = parse_bytes(input, n)?;
      Ok((input, BigInt::from_bytes_be(Sign::Minus, &inverted(bytes))))
    } else if tag == super::POSINTEND {
      let (input, raw_length) = parse_byte(input)?;
      let n: usize = usize::from(raw_length);
      let (input, bytes) = parse_bytes(input, n)?;
      Ok((input, BigInt::from_bytes_be(Sign::Plus, bytes)))
    } else {
      Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("unknown bigint tag: {}", tag),
      ))
    }
  }

  fn invert(bytes: &mut [u8]) {
    // The ones' complement of a binary number is defined as the value
    // obtained by inverting all the bits in the binary representation
    // of the number (swapping 0s for 1s and vice versa).
    for byte in bytes.iter_mut() {
      *byte = !*byte;
    }
  }

  fn inverted(bytes: &[u8]) -> Vec<u8> {
    // The ones' complement of a binary number is defined as the value
    // obtained by inverting all the bits in the binary representation
    // of the number (swapping 0s for 1s and vice versa).
    bytes.iter().map(|byte| !*byte).collect()
  }

  fn bigint_n(n: usize) -> std::io::Result<u8> {
    u8::try_from(n).map_err(|_| {
      std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "BigUint requires more than 255 bytes to be represented",
      )
    })
  }
}

mod double {
  macro_rules! sign_bit {
    ($type:ident) => {
      (1 << (std::mem::size_of::<$type>() * 8 - 1))
    };
  }

  fn f64_to_ux_be_bytes(f: f64) -> [u8; 8] {
    let u = if f.is_sign_negative() {
      f.to_bits() ^ ::std::u64::MAX
    } else {
      f.to_bits() ^ sign_bit!(u64)
    };
    u.to_be_bytes()
  }

  pub fn encode_into(out: &mut Vec<u8>, x: f64) {
    out.push(super::DOUBLE);
    out.extend_from_slice(&f64_to_ux_be_bytes(super::canonicalize_f64(x)));
  }

  pub fn decode_from(input: &[u8]) -> std::io::Result<(&[u8], f64)> {
    let (input, bytes) = super::parse_bytes(input, 8)?;
    let mut arr = [0u8; 8];
    arr.copy_from_slice(bytes);
    let u = u64::from_be_bytes(arr);
    Ok((
      input,
      f64::from_bits(if (u & sign_bit!(u64)) == 0 {
        u ^ ::std::u64::MAX
      } else {
        u ^ sign_bit!(u64)
      }),
    ))
  }
}

#[inline]
fn parse_bytes(input: &[u8], num: usize) -> std::io::Result<(&[u8], &[u8])> {
  if input.len() < num {
    Err(std::io::ErrorKind::UnexpectedEof.into())
  } else {
    Ok((&input[num..], &input[..num]))
  }
}

#[inline]
fn parse_byte(input: &[u8]) -> std::io::Result<(&[u8], u8)> {
  if input.is_empty() {
    Err(std::io::ErrorKind::UnexpectedEof.into())
  } else {
    Ok((&input[1..], input[0]))
  }
}

fn parse_slice(input: &[u8]) -> std::io::Result<(&[u8], Vec<u8>)> {
  let mut output: Vec<u8> = Vec::new();
  let mut i = 0usize;

  while i < input.len() {
    let byte = input[i];
    i += 1;

    if byte == 0 {
      if input.get(i).copied() == Some(ESCAPE) {
        output.push(0);
        i += 1;
        continue;
      } else {
        return Ok((&input[i..], output));
      }
    }

    output.push(byte);
  }

  Err(std::io::ErrorKind::UnexpectedEof.into())
}

#[cfg(test)]
mod tests {
  use num_bigint::BigInt;
  use std::cmp::Ordering;

  use crate::Key;
  use crate::KeyPart;

  use super::decode_key;
  use super::encode_key;

  fn roundtrip(key: Key) {
    let bytes = encode_key(&key).unwrap();
    let decoded = decode_key(&bytes).unwrap();
    assert_eq!(&key, &decoded);
    assert_eq!(format!("{:?}", key), format!("{:?}", decoded));
  }

  fn check_order(a: Key, b: Key, expected: Ordering) {
    let a_bytes = encode_key(&a).unwrap();
    let b_bytes = encode_key(&b).unwrap();

    assert_eq!(a.cmp(&b), expected);
    assert_eq!(a_bytes.cmp(&b_bytes), expected);
  }

  fn check_bijection(key: Key, serialized: &[u8]) {
    let bytes = encode_key(&key).unwrap();
    assert_eq!(&bytes[..], serialized);
    let decoded = decode_key(serialized).unwrap();
    assert_eq!(&key, &decoded);
  }

  #[test]
  fn simple_roundtrip() {
    roundtrip(Key(vec![
      KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00]),
      KeyPart::String("foo".to_string()),
      KeyPart::Float(-f64::NAN),
      KeyPart::Float(-f64::INFINITY),
      KeyPart::Float(-42.1),
      KeyPart::Float(-0.0),
      KeyPart::Float(0.0),
      KeyPart::Float(42.1),
      KeyPart::Float(f64::INFINITY),
      KeyPart::Float(f64::NAN),
      KeyPart::Int(BigInt::from(-10000)),
      KeyPart::Int(BigInt::from(-1)),
      KeyPart::Int(BigInt::from(0)),
      KeyPart::Int(BigInt::from(1)),
      KeyPart::Int(BigInt::from(10000)),
      KeyPart::False,
      KeyPart::True,
    ]));
  }

  #[test]
  #[rustfmt::skip]
  fn order_bytes() {
    check_order(
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00])]),
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00])]),
      Ordering::Equal,
    );

    check_order(
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00])]),
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x01])]),
      Ordering::Less,
    );

    check_order(
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x01])]),
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00])]),
      Ordering::Greater,
    );

    check_order(
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00])]),
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00, 0x00])]),
      Ordering::Less,
    );

    check_order(
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00, 0x00])]),
      Key(vec![KeyPart::Bytes(vec![0, 1, 2, 3, 0xff, 0x00, 0xff, 0x00])]),
      Ordering::Greater,
    );
  }

  #[test]
  #[rustfmt::skip]
  fn order_tags() {
    check_order(
      Key(vec![KeyPart::Bytes(vec![])]),
      Key(vec![KeyPart::String("".into())]),
      Ordering::Less,
    );

    check_order(
      Key(vec![KeyPart::String("".into())]),
      Key(vec![KeyPart::Int(BigInt::from(0))]),
      Ordering::Less,
    );

    check_order(
      Key(vec![KeyPart::Int(BigInt::from(0))]),
      Key(vec![KeyPart::Float(0.0)]),
      Ordering::Less,
    );

    check_order(
      Key(vec![KeyPart::Float(0.0)]),
      Key(vec![KeyPart::False]),
      Ordering::Less,
    );

    check_order(
      Key(vec![KeyPart::False]),
      Key(vec![KeyPart::True]),
      Ordering::Less,
    );

    check_order(
      Key(vec![KeyPart::True]),
      Key(vec![KeyPart::Bytes(vec![])]),
      Ordering::Greater,
    );
  }

  #[test]
  #[rustfmt::skip]
  fn order_floats() {
    check_order(
      Key(vec![KeyPart::Float(-f64::NAN)]),
      Key(vec![KeyPart::Float(-f64::INFINITY)]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Float(-f64::INFINITY)]),
      Key(vec![KeyPart::Float(-10.0)]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Float(-10.0)]),
      Key(vec![KeyPart::Float(-0.0)]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Float(-0.0)]),
      Key(vec![KeyPart::Float(0.0)]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Float(0.0)]),
      Key(vec![KeyPart::Float(10.0)]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Float(10.0)]),
      Key(vec![KeyPart::Float(f64::INFINITY)]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Float(f64::INFINITY)]),
      Key(vec![KeyPart::Float(f64::NAN)]),
      Ordering::Less,
    );
  }

  #[test]
  #[rustfmt::skip]
  fn order_ints() {
    check_order(
      Key(vec![KeyPart::Int(BigInt::from(-10000))]),
      Key(vec![KeyPart::Int(BigInt::from(-100))]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Int(BigInt::from(-100))]),
      Key(vec![KeyPart::Int(BigInt::from(-1))]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Int(BigInt::from(-1))]),
      Key(vec![KeyPart::Int(BigInt::from(0))]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Int(BigInt::from(0))]),
      Key(vec![KeyPart::Int(BigInt::from(1))]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Int(BigInt::from(1))]),
      Key(vec![KeyPart::Int(BigInt::from(100))]),
      Ordering::Less,
    );
    check_order(
      Key(vec![KeyPart::Int(BigInt::from(100))]),
      Key(vec![KeyPart::Int(BigInt::from(10000))]),
      Ordering::Less,
    );
  }

  #[test]
  #[rustfmt::skip]
  fn float_canonicalization() {
    let key1 = Key(vec![KeyPart::Float(f64::from_bits(0x7ff8000000000001))]);
    let key2 = Key(vec![KeyPart::Float(f64::from_bits(0x7ff8000000000002))]);

    assert_eq!(key1, key2);
    assert_eq!(encode_key(&key1).unwrap(), encode_key(&key2).unwrap());
  }

  #[test]
  #[rustfmt::skip]
  fn explicit_bijection() {
    // string
    check_bijection(
      Key(vec![KeyPart::String("hello".into())]),
      &[0x02, 0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x00],
    );

    // zero byte escape
    check_bijection(
      Key(vec![KeyPart::Bytes(vec![0x01, 0x02, 0x00, 0x07, 0x08])]),
      &[0x01, 0x01, 0x02, 0x00, 0xff, 0x07, 0x08, 0x00],
    );

    // array
    check_bijection(
      Key(vec![
        KeyPart::String("hello".into()),
        KeyPart::Bytes(vec![0x01, 0x02, 0x00, 0x07, 0x08]),
      ]),
      &[
        0x02, 0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x00, /* string */
        0x01, 0x01, 0x02, 0x00, 0xff, 0x07, 0x08, 0x00, /* bytes */
      ],
    );
  }
}
