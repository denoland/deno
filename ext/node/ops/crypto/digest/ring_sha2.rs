// Copyright 2018-2025 the Deno authors. MIT license.

use std::marker::PhantomData;

use digest::generic_array::ArrayLength;

pub trait RingDigestAlgo {
  fn algorithm() -> &'static aws_lc_rs::digest::Algorithm;
  type OutputSize: ArrayLength<u8> + 'static;
}

pub struct RingDigest<Algo: RingDigestAlgo> {
  context: aws_lc_rs::digest::Context,
  _phantom: PhantomData<Algo>,
}

impl<Algo: RingDigestAlgo> Clone for RingDigest<Algo> {
  fn clone(&self) -> Self {
    Self {
      context: self.context.clone(),
      _phantom: self._phantom,
    }
  }
}

impl<Algo: RingDigestAlgo> digest::HashMarker for RingDigest<Algo> {}
impl<Algo: RingDigestAlgo> Default for RingDigest<Algo> {
  fn default() -> Self {
    Self {
      context: aws_lc_rs::digest::Context::new(Algo::algorithm()),
      _phantom: PhantomData,
    }
  }
}
impl<Algo: RingDigestAlgo> digest::Reset for RingDigest<Algo> {
  fn reset(&mut self) {
    self.context = aws_lc_rs::digest::Context::new(Algo::algorithm())
  }
}
impl<Algo: RingDigestAlgo> digest::Update for RingDigest<Algo> {
  fn update(&mut self, data: &[u8]) {
    self.context.update(data);
  }
}
impl<Algo: RingDigestAlgo> digest::OutputSizeUser for RingDigest<Algo> {
  type OutputSize = Algo::OutputSize;
}
impl<Algo: RingDigestAlgo> digest::FixedOutput for RingDigest<Algo> {
  fn finalize_into(self, out: &mut digest::Output<Self>) {
    let result = self.context.finish();
    out.copy_from_slice(result.as_ref());
  }
}
impl<Algo: RingDigestAlgo> digest::FixedOutputReset for RingDigest<Algo> {
  fn finalize_into_reset(&mut self, out: &mut digest::Output<Self>) {
    let context = std::mem::replace(
      &mut self.context,
      aws_lc_rs::digest::Context::new(Algo::algorithm()),
    );
    out.copy_from_slice(context.finish().as_ref());
  }
}

pub struct RingSha256Algo;
impl RingDigestAlgo for RingSha256Algo {
  fn algorithm() -> &'static aws_lc_rs::digest::Algorithm {
    &aws_lc_rs::digest::SHA256
  }

  type OutputSize = digest::typenum::U32;
}
pub struct RingSha512Algo;
impl RingDigestAlgo for RingSha512Algo {
  fn algorithm() -> &'static aws_lc_rs::digest::Algorithm {
    &aws_lc_rs::digest::SHA512
  }

  type OutputSize = digest::typenum::U64;
}

pub type RingSha256 = RingDigest<RingSha256Algo>;
pub type RingSha512 = RingDigest<RingSha512Algo>;
