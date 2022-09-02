//! Non-web API ops that deal with encoding and decoding of data.
use deno_core::ByteString;
use deno_core::op;
use deno_core::Extension;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![op_escape_html::decl()])
    .build()
}

const fn create_html_escape_table() -> [u8; 256] {
    let mut table = [0; 256];
    table[b'"' as usize] = 1;
    table[b'&' as usize] = 2;
    table[b'<' as usize] = 3;
    table[b'>' as usize] = 4;
    table
}

static HTML_ESCAPE_TABLE: [u8; 256] = create_html_escape_table();

static HTML_ESCAPES: [&[u8]; 5] = [b"", b"&quot;", b"&amp;", b"&lt;", b"&gt;"];

#[op]
fn op_escape_html(s: ByteString) -> ByteString {
  ByteString::from(unsafe { simd::escape_html(&s) })
}

#[cfg(all(target_arch = "aarch64"))]
mod simd {
    use std::{arch::aarch64::*, mem::size_of};

    // neon SIMD
    const VECTOR_SIZE: usize = size_of::<int16x8_t>();
    pub unsafe fn escape_html(bytes: &[u8]) -> Vec<u8> {
        let mut w = Vec::with_capacity(bytes.len());

        let mut mark = 0;
        let mut offset = 0;

        let upperbound = bytes.len() - VECTOR_SIZE;
        while offset < upperbound {
            let mut mask = compute_mask(bytes, offset);
            while mask != 0 {
                let ix = mask.trailing_zeros();
                let i = offset + ix as usize;
                let escape_ix = *bytes.get_unchecked(i) as usize;
                let replacement = super::HTML_ESCAPES[super::HTML_ESCAPE_TABLE[escape_ix] as usize];
                w.extend_from_slice(bytes.get_unchecked(mark..i));
                mark = i + 1; // all escaped characters are ASCII
                w.extend_from_slice(replacement);
                mask ^= mask & -mask;
            }
            offset += VECTOR_SIZE;
        }

        // Final iteration. We align the read with the end of the slice and
        // shift off the bytes at start we have already scanned.
        let mut mask = compute_mask(bytes, upperbound);
        mask >>= offset - upperbound;
        while mask != 0 {
            let ix = mask.trailing_zeros();
            let i = offset + ix as usize;
            let escape_ix = *bytes.get_unchecked(i) as usize;
            let replacement = super::HTML_ESCAPES[super::HTML_ESCAPE_TABLE[escape_ix] as usize];
            w.extend_from_slice(bytes.get_unchecked(mark..i));
            mark = i + 1; // all escaped characters are ASCII
            w.extend_from_slice(replacement);
            mask ^= mask & -mask;
        }
        w
    }

    /// Creates the lookup table for use in `compute_mask`.
    const fn create_lookup() -> [u8; 16] {
        let mut table = [0; 16];
        table[(b'<' & 0x0f) as usize] = b'<';
        table[(b'>' & 0x0f) as usize] = b'>';
        table[(b'&' & 0x0f) as usize] = b'&';
        table[(b'"' & 0x0f) as usize] = b'"';
        table[0] = 0b0111_1111;
        table
    }
    unsafe fn compute_mask(bytes: &[u8], offset: usize) -> i32 {
        debug_assert!(bytes.len() >= offset + VECTOR_SIZE);

        let table = create_lookup();
        let lookup = vld1q_u16(table.as_ptr() as *const _);
        let raw_ptr = bytes.as_ptr().offset(offset as isize) as *const _;
        let vector = vld1q_u16(raw_ptr);
        let expected = {
            let tbl = vreinterpretq_u8_u16(lookup);
            let idx = vreinterpretq_u8_u16(vector);
            vreinterpretq_u16_u8(vqtbl1q_u8(tbl, idx))
        };
        let matches = vreinterpretq_u16_u8(
            vceqq_u8(vreinterpretq_u8_u16(expected), vreinterpretq_u8_u16(lookup)));
        let input = vreinterpretq_u8_u16(matches);
        let high_bits = vreinterpretq_u16_u8(vshrq_n_u8(input, 7));
        let paired16 =  vreinterpretq_u32_u16(vsraq_n_u16(high_bits, high_bits, 7));
        let paired32 =  vreinterpretq_u64_u32(vsraq_n_u32(paired16, paired16, 14));
        let paired64 =  vreinterpretq_u8_u64(vsraq_n_u64(paired32, paired32, 28));
        vgetq_lane_u8(paired64, 0) as i32 | (vgetq_lane_u8(paired64, 8) as i32) << 8
    }
}