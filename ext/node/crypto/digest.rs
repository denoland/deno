// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use digest::core_api::BlockSizeUser;
use digest::Digest;
use digest::DynDigest;
use digest::ExtendableOutput;
use digest::ExtendableOutputReset;
use digest::Reset;
use digest::Update;
use typenum::U32;
use typenum::U48;

/// Enum wrapper over all supported digest implementations.
#[derive(Clone)]
pub enum Context {
  Blake2b(Box<blake2::Blake2b512>),
  Blake2b256(Box<blake2::Blake2b<U32>>),
  Blake2b384(Box<blake2::Blake2b<U48>>),
  Blake2s(Box<blake2::Blake2s256>),
  Blake3(Box<blake3::Hasher>),
  Keccak224(Box<sha3::Keccak224>),
  Keccak256(Box<sha3::Keccak256>),
  Keccak384(Box<sha3::Keccak384>),
  Keccak512(Box<sha3::Keccak512>),
  Md4(Box<md4::Md4>),
  Md5(Box<md5::Md5>),
  Ripemd160(Box<ripemd::Ripemd160>),
  Sha1(Box<sha1::Sha1>),
  Sha3_224(Box<sha3::Sha3_224>),
  Sha3_256(Box<sha3::Sha3_256>),
  Sha3_384(Box<sha3::Sha3_384>),
  Sha3_512(Box<sha3::Sha3_512>),
  Sha224(Box<sha2::Sha224>),
  Sha256(Box<sha2::Sha256>),
  Sha384(Box<sha2::Sha384>),
  Sha512(Box<sha2::Sha512>),
  Shake128(Box<sha3::Shake128>),
  Shake256(Box<sha3::Shake256>),
  Tiger(Box<tiger::Tiger>),
}

use Context::*;

impl Context {
  pub fn new(algorithm_name: &str) -> Result<Context, &'static str> {
    Ok(match algorithm_name {
      "BLAKE2B" => Blake2b(Default::default()),
      "BLAKE2B-256" => Blake2b256(Default::default()),
      "BLAKE2B-384" => Blake2b384(Default::default()),
      "BLAKE2S" => Blake2s(Default::default()),
      "BLAKE3" => Blake3(Default::default()),
      "KECCAK-224" => Keccak224(Default::default()),
      "KECCAK-256" => Keccak256(Default::default()),
      "KECCAK-384" => Keccak384(Default::default()),
      "KECCAK-512" => Keccak512(Default::default()),
      "MD4" => Md4(Default::default()),
      "MD5" => Md5(Default::default()),
      "RIPEMD-160" => Ripemd160(Default::default()),
      "SHA-1" => Sha1(Default::default()),
      "SHA3-224" => Sha3_224(Default::default()),
      "SHA3-256" => Sha3_256(Default::default()),
      "SHA3-384" => Sha3_384(Default::default()),
      "SHA3-512" => Sha3_512(Default::default()),
      "SHA-224" => Sha224(Default::default()),
      "SHA-256" => Sha256(Default::default()),
      "SHA-384" => Sha384(Default::default()),
      "SHA-512" => Sha512(Default::default()),
      "SHAKE128" => Shake128(Default::default()),
      "SHAKE256" => Shake256(Default::default()),
      "TIGER" => Tiger(Default::default()),
      _ => return Err("unsupported algorithm"),
    })
  }

  /// The input block length for the algorithm, in bytes.
  pub fn input_block_length(&self) -> usize {
    // For algorithm types that implement BlockInput and have a statically
    // available BlockSize as part of their type definition, we use that value.
    fn static_block_length<T: BlockSizeUser>(_: &T) -> usize {
      <T::BlockSize as typenum::Unsigned>::to_usize()
    }

    match self {
      Blake2b(context) => static_block_length(&**context),
      Blake2b256(context) => static_block_length(&**context),
      Blake2b384(context) => static_block_length(&**context),
      Blake2s(context) => static_block_length(&**context),
      Blake3(_) => 64,
      Keccak224(context) => static_block_length(&**context),
      Keccak256(context) => static_block_length(&**context),
      Keccak384(context) => static_block_length(&**context),
      Keccak512(context) => static_block_length(&**context),
      Md4(context) => static_block_length(&**context),
      Md5(context) => static_block_length(&**context),
      Ripemd160(context) => static_block_length(&**context),
      Sha1(context) => static_block_length(&**context),
      Sha3_224(context) => static_block_length(&**context),
      Sha3_256(context) => static_block_length(&**context),
      Sha3_384(context) => static_block_length(&**context),
      Sha3_512(context) => static_block_length(&**context),
      Sha224(context) => static_block_length(&**context),
      Sha256(context) => static_block_length(&**context),
      Sha384(context) => static_block_length(&**context),
      Sha512(context) => static_block_length(&**context),
      Tiger(context) => static_block_length(&**context),

      // https://doi.org/10.6028/NIST.FIPS.202 specifies that:
      // - In general, the input block size (in bits) of a sponge function is
      //   its rate.
      // - SPONGE[f, pad, r] = The sponge function in which the underlying
      //   function is f, the padding rule is pad, and the rate is r.
      // - KECCAK[c] = SPONGE[KECCAK-p[1600, 24], pad10*1, 1600–c]
      // - SHAKE128(M, d) = KECCAK[256] (M || 1111, d)
      Shake128(_) => 168, // implying a rate of (1600 - 256) bits = 168 bytes
      // - SHAKE256(M, d) = KECCAK[512] (M || 1111, d).
      Shake256(_) => 136, // implying a rate of (1600 - 512) bits = 136 bytes
    }
  }

  /// The output digest length for the algorithm, in bytes.
  ///
  /// If the algorithm is variable-length, this returns its default length.
  pub fn output_length(&self) -> usize {
    match self {
      Blake2b(context) => context.output_size(),
      Blake2b256(context) => context.output_size(),
      Blake2b384(context) => context.output_size(),
      Blake2s(context) => context.output_size(),
      Blake3(context) => context.output_size(),
      Keccak224(context) => context.output_size(),
      Keccak256(context) => context.output_size(),
      Keccak384(context) => context.output_size(),
      Keccak512(context) => context.output_size(),
      Md4(context) => context.output_size(),
      Md5(context) => context.output_size(),
      Ripemd160(context) => context.output_size(),
      Sha1(context) => context.output_size(),
      Sha3_224(context) => context.output_size(),
      Sha3_256(context) => context.output_size(),
      Sha3_384(context) => context.output_size(),
      Sha3_512(context) => context.output_size(),
      Sha224(context) => context.output_size(),
      Sha256(context) => context.output_size(),
      Sha384(context) => context.output_size(),
      Sha512(context) => context.output_size(),
      Tiger(context) => context.output_size(),

      // https://doi.org/10.6028/NIST.FIPS.202's Table 4 indicates that in order
      // to reach the target security strength for these algorithms, the output
      // size (in bits) needs to be (at least) two times larger than that
      // security strength (in bits).
      Shake128(_) => 32, // implying a length of (2 * 128) bits = 32 bytes
      Shake256(_) => 64, // implying a length of (2 * 256) bits = 64 bytes
    }
  }

  /// Whether the algorithm has an extendable variable-length digest output
  /// (whether it is an "XOF").
  pub const fn extendable(&self) -> bool {
    matches!(self, Blake3(_) | Shake128(_) | Shake256(_))
  }

  /// The name of the algorithm used by this context.
  ///
  /// Names are all uppercase (for ease of case-insensitive comparisons) and
  /// will match the name formatting used in WebCrypto if the algorithm is
  /// supported by WebCrypto, and otherwise match the formatting used in the
  /// official specification for the algorithm.
  pub const fn algorithm_name(&self) -> &'static str {
    match self {
      Blake2b(_) => "BLAKE2B",
      Blake2b256(_) => "BLAKE2B-256",
      Blake2b384(_) => "BLAKE2B-384",
      Blake2s(_) => "BLAKE2S",
      Blake3(_) => "BLAKE3",
      Keccak224(_) => "KECCAK-224",
      Keccak256(_) => "KECCAK-256",
      Keccak384(_) => "KECCAK-384",
      Keccak512(_) => "KECCAK-512",
      Md4(_) => "MD5",
      Md5(_) => "MD5",
      Ripemd160(_) => "RIPEMD-160",
      Sha1(_) => "SHA-1",
      Sha3_224(_) => "SHA3-224",
      Sha3_256(_) => "SHA3-256",
      Sha3_384(_) => "SHA3-384",
      Sha3_512(_) => "SHA3-512",
      Sha224(_) => "SHA-224",
      Sha256(_) => "SHA-256",
      Sha384(_) => "SHA-384",
      Sha512(_) => "SHA-512",
      Shake128(_) => "SHAKE128",
      Shake256(_) => "SHAKE256",
      Tiger(_) => "TIGER",
    }
  }

  pub fn reset(&mut self) {
    match self {
      Blake2b(context) => Reset::reset(&mut **context),
      Blake2b256(context) => Reset::reset(&mut **context),
      Blake2b384(context) => Reset::reset(&mut **context),
      Blake2s(context) => Reset::reset(&mut **context),
      Blake3(context) => Reset::reset(&mut **context),
      Keccak224(context) => Reset::reset(&mut **context),
      Keccak256(context) => Reset::reset(&mut **context),
      Keccak384(context) => Reset::reset(&mut **context),
      Keccak512(context) => Reset::reset(&mut **context),
      Md4(context) => Reset::reset(&mut **context),
      Md5(context) => Reset::reset(&mut **context),
      Ripemd160(context) => Reset::reset(&mut **context),
      Sha1(context) => Reset::reset(&mut **context),
      Sha3_224(context) => Reset::reset(&mut **context),
      Sha3_256(context) => Reset::reset(&mut **context),
      Sha3_384(context) => Reset::reset(&mut **context),
      Sha3_512(context) => Reset::reset(&mut **context),
      Sha224(context) => Reset::reset(&mut **context),
      Sha256(context) => Reset::reset(&mut **context),
      Sha384(context) => Reset::reset(&mut **context),
      Sha512(context) => Reset::reset(&mut **context),
      Shake128(context) => Reset::reset(&mut **context),
      Shake256(context) => Reset::reset(&mut **context),
      Tiger(context) => Reset::reset(&mut **context),
    };
  }

  pub fn update(&mut self, data: &[u8]) {
    match self {
      Blake2b(context) => Digest::update(&mut **context, data),
      Blake2b256(context) => Digest::update(&mut **context, data),
      Blake2b384(context) => Digest::update(&mut **context, data),
      Blake2s(context) => Digest::update(&mut **context, data),
      Blake3(context) => Digest::update(&mut **context, data),
      Keccak224(context) => Digest::update(&mut **context, data),
      Keccak256(context) => Digest::update(&mut **context, data),
      Keccak384(context) => Digest::update(&mut **context, data),
      Keccak512(context) => Digest::update(&mut **context, data),
      Md4(context) => Digest::update(&mut **context, data),
      Md5(context) => Digest::update(&mut **context, data),
      Ripemd160(context) => Digest::update(&mut **context, data),
      Sha1(context) => Digest::update(&mut **context, data),
      Sha3_224(context) => Digest::update(&mut **context, data),
      Sha3_256(context) => Digest::update(&mut **context, data),
      Sha3_384(context) => Digest::update(&mut **context, data),
      Sha3_512(context) => Digest::update(&mut **context, data),
      Sha224(context) => Digest::update(&mut **context, data),
      Sha256(context) => Digest::update(&mut **context, data),
      Sha384(context) => Digest::update(&mut **context, data),
      Sha512(context) => Digest::update(&mut **context, data),
      Tiger(context) => Digest::update(&mut **context, data),
      Shake128(context) => (&mut **context).update(data),
      Shake256(context) => (&mut **context).update(data),
    };
  }

  pub fn digest_and_drop(
    self,
    length: Option<usize>,
  ) -> Result<Box<[u8]>, &'static str> {
    if let Some(length) = length {
      if !self.extendable() && length != self.output_length() {
        return Err(
          "non-default length specified for non-extendable algorithm",
        );
      }
    }

    let length = length.unwrap_or_else(|| self.output_length());

    Ok(match self {
      Blake2b(context) => context.finalize(),
      Blake2b256(context) => context.finalize(),
      Blake2b384(context) => context.finalize(),
      Blake2s(context) => context.finalize(),
      Blake3(context) => context.finalize_boxed(length),
      Keccak224(context) => context.finalize(),
      Keccak256(context) => context.finalize(),
      Keccak384(context) => context.finalize(),
      Keccak512(context) => context.finalize(),
      Md4(context) => context.finalize(),
      Md5(context) => context.finalize(),
      Ripemd160(context) => context.finalize(),
      Sha1(context) => context.finalize(),
      Sha3_224(context) => context.finalize(),
      Sha3_256(context) => context.finalize(),
      Sha3_384(context) => context.finalize(),
      Sha3_512(context) => context.finalize(),
      Sha224(context) => context.finalize(),
      Sha256(context) => context.finalize(),
      Sha384(context) => context.finalize(),
      Sha512(context) => context.finalize(),
      Tiger(context) => context.finalize(),
      Shake128(context) => context.finalize_boxed(length),
      Shake256(context) => context.finalize_boxed(length),
    })
  }

  pub fn digest_and_reset(
    &mut self,
    length: Option<usize>,
  ) -> Result<Box<[u8]>, &'static str> {
    if let Some(length) = length {
      if !self.extendable() && length != self.output_length() {
        return Err(
          "non-default length specified for non-extendable algorithm",
        );
      }
    }

    let length = length.unwrap_or_else(|| self.output_length());
    Ok(match self {
      Blake2b(context) => DynDigest::finalize_reset(context.as_mut()),
      Blake2b256(context) => DynDigest::finalize_reset(context.as_mut()),
      Blake2b384(context) => DynDigest::finalize_reset(context.as_mut()),
      Blake2s(context) => DynDigest::finalize_reset(context.as_mut()),
      Blake3(context) => {
        ExtendableOutputReset::finalize_boxed_reset(context.as_mut(), length)
      }
      Keccak224(context) => DynDigest::finalize_reset(context.as_mut()),
      Keccak256(context) => DynDigest::finalize_reset(context.as_mut()),
      Keccak384(context) => DynDigest::finalize_reset(context.as_mut()),
      Keccak512(context) => DynDigest::finalize_reset(context.as_mut()),
      Md4(context) => DynDigest::finalize_reset(context.as_mut()),
      Md5(context) => DynDigest::finalize_reset(context.as_mut()),
      Ripemd160(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha1(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha3_224(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha3_256(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha3_384(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha3_512(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha224(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha256(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha384(context) => DynDigest::finalize_reset(context.as_mut()),
      Sha512(context) => DynDigest::finalize_reset(context.as_mut()),
      Tiger(context) => DynDigest::finalize_reset(context.as_mut()),
      Shake128(context) => {
        ExtendableOutputReset::finalize_boxed_reset(context.as_mut(), length)
      }
      Shake256(context) => {
        ExtendableOutputReset::finalize_boxed_reset(context.as_mut(), length)
      }
    })
  }

  pub fn digest(
    &self,
    length: Option<usize>,
  ) -> Result<Box<[u8]>, &'static str> {
    self.clone().digest_and_drop(length)
  }
}
