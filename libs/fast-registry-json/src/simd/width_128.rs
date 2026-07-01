// Copyright 2018-2026 the Deno authors. MIT license.

// All adapted from https://github.com/simdjson/simdjson
#[cfg(all(
  feature = "simd",
  target_arch = "aarch64",
  target_feature = "neon"
))]
use std::arch::aarch64::uint8x16_t;
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
use std::arch::x86_64::__m128i;
use std::marker::PhantomData;
use std::ops::BitAnd;
use std::ops::BitOr;
use std::ops::BitXor;
use std::ops::Not;

use arrayref::array_refs;
use arrayref::mut_array_refs;
use wide::u8x16;

use crate::pick;

#[derive(Copy, Clone)]
pub struct Simd8<T> {
  pub base: u8x16,
  _marker: PhantomData<T>,
}

impl<T> Simd8<T> {
  #[inline(always)]
  fn from_base(base: u8x16) -> Self {
    Self {
      base,
      _marker: PhantomData,
    }
  }

  #[inline(always)]
  pub fn eq_mask(&self, rhs: &Simd8<T>) -> Simd8<bool> {
    self.base.cmp_eq(rhs.base).into()
  }
}

impl<T> BitOr for Simd8<T> {
  type Output = Self;

  #[inline(always)]
  fn bitor(self, rhs: Self) -> Self::Output {
    Self::from_base(self.base | rhs.base)
  }
}

impl<T> BitAnd for Simd8<T> {
  type Output = Self;

  #[inline(always)]
  fn bitand(self, rhs: Self) -> Self::Output {
    Self::from_base(self.base & rhs.base)
  }
}

impl<T> BitXor for Simd8<T> {
  type Output = Self;

  #[inline(always)]
  fn bitxor(self, rhs: Self) -> Self::Output {
    Self::from_base(self.base ^ rhs.base)
  }
}

impl<T> Not for Simd8<T> {
  type Output = Self;

  #[inline(always)]
  fn not(self) -> Self::Output {
    Self::from_base(!self.base)
  }
}

impl<T> From<u8x16> for Simd8<T> {
  #[inline(always)]
  fn from(value: u8x16) -> Self {
    Self::from_base(value)
  }
}

impl Simd8<u8> {
  #[inline(always)]
  /// Loads 16 bytes into a vector.
  pub fn load(values: [u8; 16]) -> u8x16 {
    u8x16::new(values)
  }
  #[inline(always)]
  pub fn zero() -> u8x16 {
    u8x16::ZERO
  }
  #[inline(always)]
  pub fn splat(value: u8) -> Self {
    u8x16::splat(value).into()
  }

  #[inline(always)]
  pub fn store(&self, dst: &mut [u8; 16]) {
    dst.copy_from_slice(self.base.as_array_ref());
  }
}

impl From<u8> for Simd8<u8> {
  #[inline(always)]
  fn from(value: u8) -> Self {
    Self::splat(value)
  }
}

impl From<&'_ [u8; 16]> for Simd8<u8> {
  #[inline(always)]
  fn from(value: &'_ [u8; 16]) -> Self {
    Self::load(*value).into()
  }
}
impl From<[u8; 16]> for Simd8<u8> {
  #[inline(always)]
  fn from(value: [u8; 16]) -> Self {
    Self::load(value).into()
  }
}

impl Simd8<bool> {
  #[inline(always)]
  pub fn to_bitmask(&self) -> u32 {
    pick! {
        if #[cfg(all(feature = "simd", target_arch = "x86_64"))] {
            let inner: __m128i = bytemuck::must_cast(self.base);
            // SAFETY: SSE2 is part of the x86_64 baseline, and `inner` is
            // a valid 128-bit vector value.
            unsafe { std::arch::x86_64::_mm_movemask_epi8(inner) as u32 }
        } else {
            let bit_mask = u8x16::new([
                0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80, 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80,
            ]);
            let m_input = self.base & bit_mask;
            let mut tmp = m_input + m_input;
            tmp = tmp + tmp;
            tmp = tmp + tmp;
            bytemuck::must_cast::<u8x16, wide::u16x8>(tmp).as_array_ref()[0] as u32
        }
    }
  }
}

#[allow(
  clippy::too_many_arguments,
  reason = "constructs a 16-byte vector literal"
)]
#[inline(always)]
pub fn make_u8x16(
  a: u8,
  b: u8,
  c: u8,
  d: u8,
  e: u8,
  f: u8,
  g: u8,
  h: u8,
  i: u8,
  j: u8,
  k: u8,
  l: u8,
  m: u8,
  n: u8,
  o: u8,
  p: u8,
) -> u8x16 {
  let array = [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p];
  u8x16::new(array)
}
const NUM_CHUNKS: usize = 64 / size_of::<Simd8<u8>>();

pub struct Simd8x64<T> {
  pub chunks: [Simd8<T>; NUM_CHUNKS],
}

impl<T> Simd8x64<T> {
  #[inline(always)]
  pub fn from_chunks(chunks: [Simd8<T>; NUM_CHUNKS]) -> Self {
    Self { chunks }
  }
}

impl Simd8x64<u8> {
  #[inline(always)]
  pub fn store(&self, buf: &mut [u8; 64]) {
    let (a, b, c, d) = mut_array_refs![buf, 16, 16, 16, 16];
    self.chunks[0].store(a);
    self.chunks[1].store(b);
    self.chunks[2].store(c);
    self.chunks[3].store(d);
  }

  #[inline(always)]
  pub fn load(buf: &[u8; 64]) -> Self {
    let (a, b, c, d) = array_refs![buf, 16, 16, 16, 16];
    Self {
      chunks: [a.into(), b.into(), c.into(), d.into()],
    }
  }
}

impl Simd8x64<u8> {
  #[inline(always)]
  pub fn eq(&self, value: u8) -> u64 {
    let mask = Simd8::<u8>::splat(value);

    let a = mask.eq_mask(&self.chunks[0]);
    let b = mask.eq_mask(&self.chunks[1]);
    let c = mask.eq_mask(&self.chunks[2]);
    let d = mask.eq_mask(&self.chunks[3]);

    Simd8x64::<bool> {
      chunks: [a, b, c, d],
    }
    .to_bitmask()
  }
}

impl<T> Simd8x64<T> {
  #[inline(always)]
  pub fn cmp_eq_mask(&self, other: &Simd8x64<T>) -> u64 {
    let a = self.chunks[0].eq_mask(&other.chunks[0]);
    let b = self.chunks[1].eq_mask(&other.chunks[1]);
    let c = self.chunks[2].eq_mask(&other.chunks[2]);
    let d = self.chunks[3].eq_mask(&other.chunks[3]);

    Simd8x64::<bool> {
      chunks: [a, b, c, d],
    }
    .to_bitmask()
  }
}

impl Simd8x64<bool> {
  #[inline(always)]
  pub fn to_bitmask(&self) -> u64 {
    pick! {
        if #[cfg(all(feature = "simd", target_arch = "x86_64"))] {
            let r0 = self.chunks[0].to_bitmask() as u64;
            let r1 = self.chunks[1].to_bitmask() as u64;
            let r2 = self.chunks[2].to_bitmask() as u64;
            let r3 = self.chunks[3].to_bitmask() as u64;
            r0 | (r1 << 16) | (r2 << 32) | (r3 << 48)
        } else {
            let bit_mask = make_u8x16(
                0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80, 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80,
            );
            let sum0 = (self.chunks[0].base & bit_mask).pairwise_add(self.chunks[1].base & bit_mask);
            let sum1 = (self.chunks[2].base & bit_mask).pairwise_add(self.chunks[3].base & bit_mask);
            let sum0 = sum0.pairwise_add(sum1);
            let sum0 = sum0.pairwise_add(sum0);

            bytemuck::must_cast::<u8x16, wide::u64x2>(sum0).as_array_ref()[0]
        }
    }
  }
}

impl Simd8<u8> {
  #[inline(always)]
  pub fn shr<const N: i32>(&self) -> Self {
    pick! {
        if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            let inner: uint8x16_t = bytemuck::must_cast(self.base);
            // SAFETY: this branch only compiles when NEON is enabled, and
            // `inner` is a valid 128-bit vector value.
            let shifted = unsafe { std::arch::aarch64::vshrq_n_u8(inner, N) };
            bytemuck::must_cast::<uint8x16_t, u8x16>(shifted).into()
        } else {
            let inner = self.base.as_array_ref();
            let mut arr = [0u8; 16];
            for i in 0..16 {
                arr[i] = inner[i] >> N;
            }
            u8x16::new(arr).into()
        }
    }
  }
  #[inline(always)]
  pub fn shl<const N: i32>(&self) -> Self {
    pick! {
        if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            let inner: uint8x16_t = bytemuck::must_cast(self.base);
            // SAFETY: this branch only compiles when NEON is enabled, and
            // `inner` is a valid 128-bit vector value.
            let shifted = unsafe { std::arch::aarch64::vshlq_n_u8(inner, N) };
            bytemuck::must_cast::<uint8x16_t, u8x16>(shifted).into()
        } else {
            let inner = self.base.as_array_ref();
            let mut arr = [0u8; 16];
            for i in 0..16 {
                arr[i] = inner[i] << N;
            }
            u8x16::new(arr).into()
        }
    }
  }
  #[inline(always)]
  pub fn apply_lookup_16_to(&self, original: Simd8<u8>) -> Simd8<u8> {
    pick! {
        if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            let inner: uint8x16_t = bytemuck::must_cast(self.base);
            let inner2: uint8x16_t = bytemuck::must_cast(original.base);
            // SAFETY: this branch only compiles when NEON is enabled, and
            // both operands are valid 128-bit vector values.
            let looked_up = unsafe { std::arch::aarch64::vqtbl1q_u8(inner, inner2) };
            bytemuck::must_cast::<uint8x16_t, u8x16>(looked_up).into()
        } else {
            let inner = self.base.as_array_ref();
            let inner2 = original.base.as_array_ref();
            let mut arr = [0u8; 16];
            for i in 0..16 {
                if inner2[i] < 16 {
                    arr[i] = inner[inner2[i] as usize];
                } else {
                    arr[i] = 0;
                }
            }
            u8x16::new(arr).into()
        }
    }
  }
  #[inline(always)]
  pub fn lookup_16_table(&self, table: Simd8<u8>) -> Simd8<u8> {
    table.apply_lookup_16_to(*self)
  }

  #[allow(
    clippy::too_many_arguments,
    reason = "models a 16-entry lookup table"
  )]
  #[inline(always)]
  pub fn lookup_16(
    &self,
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    g: u8,
    h: u8,
    i: u8,
    j: u8,
    k: u8,
    l: u8,
    m: u8,
    n: u8,
    o: u8,
    p: u8,
  ) -> Simd8<u8> {
    let table =
      make_u8x16(a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p).into();

    self.lookup_16_table(table)
  }

  #[inline(always)]
  pub fn any_bits_set(&self, bits: Simd8<u8>) -> Simd8<bool> {
    pick! {
        if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            let inner: uint8x16_t = bytemuck::must_cast(self.base);
            let inner2: uint8x16_t = bytemuck::must_cast(bits.base);

            // SAFETY: this branch only compiles when NEON is enabled, and
            // both operands are valid 128-bit vector values.
            let tested = unsafe { std::arch::aarch64::vtstq_u8(inner, inner2) };
            bytemuck::must_cast::<uint8x16_t, u8x16>(tested).into()
        } else {
            let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = self.base.to_array();
            let [a2, b2, c2, d2, e2, f2, g2, h2, i2, j2, k2, l2, m2, n2, o2, p2] =
                bits.base.to_array();
            u8x16::new([
                if a & a2 != 0 { 0xFF } else { 0x00 },
                if b & b2 != 0 { 0xFF } else { 0x00 },
                if c & c2 != 0 { 0xFF } else { 0x00 },
                if d & d2 != 0 { 0xFF } else { 0x00 },
                if e & e2 != 0 { 0xFF } else { 0x00 },
                if f & f2 != 0 { 0xFF } else { 0x00 },
                if g & g2 != 0 { 0xFF } else { 0x00 },
                if h & h2 != 0 { 0xFF } else { 0x00 },
                if i & i2 != 0 { 0xFF } else { 0x00 },
                if j & j2 != 0 { 0xFF } else { 0x00 },
                if k & k2 != 0 { 0xFF } else { 0x00 },
                if l & l2 != 0 { 0xFF } else { 0x00 },
                if m & m2 != 0 { 0xFF } else { 0x00 },
                if n & n2 != 0 { 0xFF } else { 0x00 },
                if o & o2 != 0 { 0xFF } else { 0x00 },
                if p & p2 != 0 { 0xFF } else { 0x00 },
            ]).into()
        }
    }
  }
}

pub trait U8x16Ext {
  fn pairwise_add(self, other: u8x16) -> u8x16;
}

impl U8x16Ext for u8x16 {
  #[inline(always)]
  fn pairwise_add(self, other: u8x16) -> u8x16 {
    pick! {
        if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
            let inner: uint8x16_t = bytemuck::must_cast(self);
            let inner2: uint8x16_t = bytemuck::must_cast(other);
            // SAFETY: this branch only compiles when NEON is enabled, and
            // both operands are valid 128-bit vector values.
            let summed = unsafe { std::arch::aarch64::vpaddq_u8(inner, inner2) };
            bytemuck::must_cast::<uint8x16_t, u8x16>(summed)
        } else {
            let arr = self.as_array_ref();
            let arr2 = other.as_array_ref();
            u8x16::new([
                arr[0] + arr[1],
                arr[2] + arr[3],
                arr[4] + arr[5],
                arr[6] + arr[7],
                arr[8] + arr[9],
                arr[10] + arr[11],
                arr[12] + arr[13],
                arr[14] + arr[15],
                arr2[0] + arr2[1],
                arr2[2] + arr2[3],
                arr2[4] + arr2[5],
                arr2[6] + arr2[7],
                arr2[8] + arr2[9],
                arr2[10] + arr2[11],
                arr2[12] + arr2[13],
                arr2[14] + arr2[15],
            ])
        }
    }
  }
}

pick! {
    if #[cfg(all(feature = "simd", target_arch = "x86_64"))] {
        impl<T> From<__m128i> for Simd8<T> {
            #[inline(always)]
            fn from(value: __m128i) -> Self {
                bytemuck::must_cast::<__m128i, u8x16>(value).into()
            }
        }
    }
}
