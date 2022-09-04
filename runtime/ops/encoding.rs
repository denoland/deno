//! Non-web API ops that deal with encoding and decoding of data.
use deno_core::op;
use deno_core::Extension;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![op_escape_html::decl()])
    .build()
}

// TODO(@littledivy): Fast path for Latin1 strings
#[op]
fn op_escape_html(s: String) -> String {
  let mut out = Vec::with_capacity(s.len());
  // TODO(@littledivy): Add ssse3 implementation for x86
  #[cfg(all(target_arch = "aarch64"))]
  {
    simd_escape(s.as_bytes(), &mut out);
  }
  #[cfg(not(all(target_arch = "aarch64")))]
  {
    fallback_escape(s.as_bytes(), &mut out);
  }
  // SAFETY: `out` is guaranteed to be valid UTF-8 string.
  unsafe { String::from_utf8_unchecked(out) }
}

const ESACPES: [u8; 5] = [b'"', b'&', b'\'', b'<', b'>'];

#[cfg(all(target_arch = "aarch64"))]
mod simd {
  use super::ESACPES;
  use std::arch::aarch64::*;

  #[inline(always)]
  pub unsafe fn find(ptr: *const u8, n: usize) -> Option<usize> {
    let first = vdupq_n_u8(ESACPES[0]);
    let second = vdupq_n_u8(ESACPES[1]);
    let third = vdupq_n_u8(ESACPES[2]);
    let fourth = vdupq_n_u8(ESACPES[3]);
    let fifth = vdupq_n_u8(ESACPES[4]);

    for i in (0..n).step_by(16) {
      let block = vld1q_u8(ptr.add(i));
      let mut eq = vceqq_u8(block, first);
      eq = vorrq_u8(eq, vceqq_u8(block, second));
      eq = vorrq_u8(eq, vceqq_u8(block, third));
      eq = vorrq_u8(eq, vceqq_u8(block, fourth));
      eq = vorrq_u8(eq, vceqq_u8(block, fifth));

      let mut mask = vgetq_lane_u64(vreinterpretq_u64_u8(eq), 0);
      if mask != 0 {
        for j in 0..8 {
          if mask & 0xff != 0 {
            return Some(i + j);
          }

          mask >>= 8;
        }
      }

      mask = vgetq_lane_u64(vreinterpretq_u64_u8(eq), 1);
      if mask != 0 {
        for j in 0..8 {
          if mask & 0xff != 0 {
            return Some(i + j + 8);
          }

          mask >>= 8;
        }
      }
    }

    None
  }
}

pub fn fallback_escape(raw: &[u8], escaped: &mut Vec<u8>) {
  let mut pos = 0;
  while let Some(i) = raw[pos..].iter().position(|&r| ESACPES.contains(&r)) {
    let new_pos = pos + i;
    escaped.extend_from_slice(&raw[pos..new_pos]);
    match raw[new_pos] {
      b'<' => escaped.extend_from_slice(b"&lt;"),
      b'>' => escaped.extend_from_slice(b"&gt;"),
      b'\'' => escaped.extend_from_slice(b"&apos;"),
      b'&' => escaped.extend_from_slice(b"&amp;"),
      b'"' => escaped.extend_from_slice(b"&quot;"),
      c => unreachable!(
        "Found {} but only '<', '>', ', '&' and '\"' are escaped",
        c as char
      ),
    }
    pos = new_pos + 1;
  }

  if let Some(raw) = raw.get(pos..) {
    escaped.extend_from_slice(raw);
  }
}

#[cfg(all(target_arch = "aarch64"))]
pub fn simd_escape(raw: &[u8], escaped: &mut Vec<u8>) {
  let mut pos = 0;
  while let Some(i) =
    // SAFETY: the resulting pointer is within bounds. n is also within bounds from
    // the resulting pointer.
    unsafe { simd::find(raw.as_ptr().add(pos), raw.len() - pos) }
  {
    let new_pos = pos + i;
    escaped.extend_from_slice(&raw[pos..new_pos]);
    match raw[new_pos] {
      b'<' => escaped.extend_from_slice(b"&lt;"),
      b'>' => escaped.extend_from_slice(b"&gt;"),
      b'\'' => escaped.extend_from_slice(b"&apos;"),
      b'&' => escaped.extend_from_slice(b"&amp;"),
      b'"' => escaped.extend_from_slice(b"&quot;"),
      c => unreachable!(
        "Found {} but only '<', '>', ', '&' and '\"' are escaped",
        c as char
      ),
    }
    pos = new_pos + 1;
  }

  if let Some(raw) = raw.get(pos..) {
    escaped.extend_from_slice(raw);
  }
}
