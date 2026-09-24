// Copyright 2018-2026 the Deno authors. MIT license.

use ed448_goldilocks::EdwardsScalar;
use ed448_goldilocks::MontgomeryPoint;
use ed448_goldilocks::elliptic_curve::bigint::U448;
use ed448_goldilocks::elliptic_curve::scalar::FromUintUnchecked;

pub const BASE_POINT: [u8; 56] = {
  let mut point = [0; 56];
  point[0] = 5;
  point
};

/// The X448 function from RFC 7748: decode and clamp the scalar `k`
/// (section 5) and perform the Montgomery ladder against the point `u`.
///
/// The clamped scalar has its top bit (bit 447) set, so it exceeds the
/// Ed448 group order. It must therefore be used verbatim rather than
/// reduced mod order.
pub fn x448(k: &[u8; 56], u: &[u8; 56]) -> [u8; 56] {
  // decodeScalar448 (RFC 7748, section 5).
  let mut scalar_bytes = *k;
  scalar_bytes[0] &= 252;
  scalar_bytes[55] |= 128;
  let scalar =
    EdwardsScalar::from_uint_unchecked(U448::from_le_slice(&scalar_bytes));
  (&MontgomeryPoint(*u) * &scalar).0
}

#[cfg(test)]
mod tests {
  use super::*;

  fn from_hex<const N: usize>(value: &str) -> [u8; N] {
    let bytes: Vec<_> = (0..value.len())
      .step_by(2)
      .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
      .collect();
    bytes.try_into().unwrap()
  }

  #[test]
  fn public_key_from_scalar() {
    let scalar = from_hex(
      "27a4354608f3bdd38f1f5af305f3e0682efe4e25808249d8fcb55927f6a9f446b8dc1d0a2c3b8cb133a5673b59a6d55ce754ec0c9a555401",
    );
    let expected = from_hex(
      "145d083ea7a6379dbb32dcbd8aff4c206ea5d069b75e96c6dd2a3e38f441471ac97adca641fdad66685a96f32b7c3e064635fab3cc89234e",
    );

    assert_eq!(x448(&scalar, &BASE_POINT), expected);
  }

  #[test]
  fn rfc7748_ecdh() {
    // RFC 7748 section 5.2 X448 test vector.
    let scalar = from_hex(
      "3d262fddf9ec8e88495266fea19a34d28882acef045104d0d1aae121700a779c984c24f8cdd78fbff44943eba368f54b29259a4f1c600ad3",
    );
    let u = from_hex(
      "06fce640fa3487bfda5f6cf2d5263f8aad88334cbd07437f020f08f9814dc031ddbdc38c19c6da2583fa5429db94ada18aa7a7fb4ef8a086",
    );
    let expected = from_hex(
      "ce3e4ff95a60dc6697da1db1d85e6afbdf79b50a2412d7546d5f239fe14fbaadeb445fc66a01b0779d98223961111e21766282f73dd96b6f",
    );

    assert_eq!(x448(&scalar, &u), expected);
  }
}
