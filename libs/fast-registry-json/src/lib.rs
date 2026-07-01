// Copyright 2018-2026 the Deno authors. MIT license.

// JSON scanning adapted from the "stage 1" of https://github.com/simdjson/simdjson, though it diverges
// to optimize for this specific use case.
pub mod simd;

// lifted from `wide`
macro_rules! pick {
    ($(if #[cfg($($test:meta),*)] {
        $($if_tokens:tt)*
        })else+ else {
        $($else_tokens:tt)*
        }) => {
        pick!{
        @__forests [ ] ;
        $( [ {$($test),*} {$($if_tokens)*} ], )*
        [ { } {$($else_tokens)*} ],
        }
    };
    (if #[cfg($($if_meta:meta),*)] {
        $($if_tokens:tt)*
        } $(else if #[cfg($($else_meta:meta),*)] {
        $($else_tokens:tt)*
        })*) => {
        pick!{
        @__forests [ ] ;
        [ {$($if_meta),*} {$($if_tokens)*} ],
        $( [ {$($else_meta),*} {$($else_tokens)*} ], )*
        }
    };
    (@__forests [$($not:meta,)*];) => {
        /* halt expansion */
    };
    (@__forests [$($not:meta,)*]; [{$($m:meta),*} {$($tokens:tt)*}], $($rest:tt)*) => {
        #[cfg(all( $($m,)* not(any($($not),*)) ))]
        pick!{ @__identity $($tokens)* }
        pick!{ @__forests [ $($not,)* $($m,)* ] ; $($rest)* }
    };
    (@__identity $($tokens:tt)*) => {
        $($tokens)*
    };
}

pub(crate) use pick;

pub struct BufBlockReader<'a, const STEP_SIZE: usize> {
  buf: &'a [u8],
  len_minus_step: usize,
  idx: usize,
}

impl<'a, const STEP_SIZE: usize> BufBlockReader<'a, STEP_SIZE> {
  pub fn new(buf: &'a [u8]) -> Self {
    Self {
      len_minus_step: buf.len().saturating_sub(STEP_SIZE),
      idx: 0,
      buf,
    }
  }

  pub fn has_full_block(&self) -> bool {
    self.idx < self.len_minus_step
  }

  pub fn full_block(&self) -> &'a [u8] {
    &self.buf[self.idx..]
  }

  pub fn get_remainder(&self, dest: &mut [u8]) -> usize {
    if self.buf.len() == self.idx {
      return 0;
    }

    dest[..STEP_SIZE].fill(0x20);
    let remainder = &self.buf[self.idx..];
    dest[..remainder.len()].copy_from_slice(remainder);
    self.buf.len() - self.idx
  }

  pub fn advance(&mut self) {
    self.idx += STEP_SIZE;
  }
}

#[derive(Default)]
struct JsonEscapeScanner {
  next_is_escaped: u64,
}

struct EscapedAndEscape {
  /// Mask of characters escaped by a preceding backslash. This excludes the
  /// backslashes that perform the escaping.
  escaped: u64,
}

impl JsonEscapeScanner {
  fn new() -> Self {
    Self::default()
  }

  fn next(&mut self, backslash: u64) -> EscapedAndEscape {
    // Work from runs of `\` characters. Even-length runs end unescaped;
    // odd-length runs escape the following byte. A trailing odd run carries
    // into the next block through `next_is_escaped`.
    let escape_and_terminal_code =
      Self::next_escape_and_terminal_code(backslash & !self.next_is_escaped);
    let escaped = escape_and_terminal_code ^ (backslash | self.next_is_escaped);
    let escape = escape_and_terminal_code & backslash;
    self.next_is_escaped = escape >> 63;
    EscapedAndEscape { escaped }
  }

  fn next_escape_and_terminal_code(potential_escape: u64) -> u64 {
    // Shift the candidate backslashes to the following byte, then use
    // subtraction plus an odd-bit mask to distinguish odd and even aligned
    // runs. The final xor leaves each escaping backslash and the byte it
    // escapes set in the returned mask.
    let maybe_escaped = potential_escape << 1;

    const ODD_BITS: u64 = 0xAAAAAAAAAAAAAAAA;

    let maybe_escaped_and_odd_bits = maybe_escaped | ODD_BITS;
    let even_series_codes_and_odd_bits =
      maybe_escaped_and_odd_bits.wrapping_sub(potential_escape);

    even_series_codes_and_odd_bits ^ ODD_BITS
  }
}

/// A block of JSON string processing results
#[derive(Debug)]
pub struct JsonStringBlock {
  // Escaped characters (characters following an escape character)
  escaped: u64,
  // Real (non-backslashed) quotes
  quote: u64,
  // String characters (includes start quote but not end quote)
  in_string: u64,
}

impl JsonStringBlock {
  pub fn new(escaped: u64, quote: u64, in_string: u64) -> Self {
    Self {
      escaped,
      quote,
      in_string,
    }
  }

  // Escaped characters (characters following an escape character)
  pub fn escaped(&self) -> u64 {
    self.escaped
  }

  // Real (non-backslashed) quotes
  pub fn quote(&self) -> u64 {
    self.quote
  }

  // Only characters inside the string (not including the quotes)
  pub fn string_content(&self) -> u64 {
    self.in_string & !self.quote
  }

  // Return a mask of whether the given characters are inside a string (only works on non-quotes)
  pub fn non_quote_inside_string(&self, mask: u64) -> u64 {
    mask & self.in_string
  }

  // Return a mask of whether the given characters are outside a string (only works on non-quotes)
  pub fn non_quote_outside_string(&self, mask: u64) -> u64 {
    mask & !self.in_string
  }

  // Tail of string (everything except the start quote)
  pub fn string_tail(&self) -> u64 {
    self.in_string ^ self.quote
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
  InputTooLong,
  UnclosedString,
  UnmatchedBrace(usize),
}

pub struct JsonStringScanner {
  // Scans for escape characters
  escape_scanner: JsonEscapeScanner,
  // Whether the last iteration was still inside a string (all 1's = true, all 0's = false).
  prev_in_string: u64,
}

impl Default for JsonStringScanner {
  fn default() -> Self {
    Self::new()
  }
}

impl JsonStringScanner {
  pub fn new() -> Self {
    Self {
      escape_scanner: JsonEscapeScanner::new(),
      prev_in_string: 0,
    }
  }

  /// Return a mask of all string characters plus end quotes.
  ///
  /// prev_escaped is overflow saying whether the next character is escaped.
  /// prev_in_string is overflow saying whether we're still in a string.
  ///
  /// Backslash sequences outside of quotes will be detected in stage 2.
  pub fn next(&mut self, input: &simd::Simd8x64<u8>) -> JsonStringBlock {
    let backslash = input.eq(b'\\');
    let escaped = self.escape_scanner.next(backslash).escaped;
    let quote = input.eq(b'"') & !escaped;

    // prefix_xor flips on bits inside the string (and flips off the end quote).
    // Then we xor with prev_in_string: if we were in a string already, its effect is flipped
    // (characters inside strings are outside, and characters outside strings are inside).
    let in_string = prefix_xor(quote) ^ self.prev_in_string;

    // Check if we're still in a string at the end of the box so the next block will know
    self.prev_in_string = (in_string as i64).wrapping_shr(63) as u64;

    JsonStringBlock::new(escaped, quote, in_string)
  }

  /// Returns either UnclosedString or Success
  pub fn finish(&self) -> Result<(), Error> {
    if self.prev_in_string != 0 {
      Err(Error::UnclosedString)
    } else {
      Ok(())
    }
  }
}

/// Performs a prefix XOR operation on a 64-bit value
///
/// This is equivalent to the prefix_xor function in the C++ code
fn prefix_xor(mask: u64) -> u64 {
  let mut result = mask;
  // Prefix XOR each bit with the previous bits
  result ^= result << 1;
  result ^= result << 2;
  result ^= result << 4;
  result ^= result << 8;
  result ^= result << 16;
  result ^= result << 32;
  result
}

/// A block of JSON character classification results
#[derive(Debug)]
pub struct JsonCharacterBlock {
  whitespace: u64,
  op: u64,
}

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
type CharacterClassifier = fn(&simd::Simd8x64<u8>) -> JsonCharacterBlock;

mod classify {
  use super::*;

  #[inline(always)]
  #[allow(dead_code, reason = "fallback classifier used on some targets")]
  pub fn classify_by_comparison(
    input: &simd::Simd8x64<u8>,
  ) -> JsonCharacterBlock {
    let whitespace =
      input.eq(b' ') | input.eq(b'\t') | input.eq(b'\n') | input.eq(b'\r');
    let op = input.eq(b',')
      | input.eq(b':')
      | input.eq(b'[')
      | input.eq(b'{')
      | input.eq(b']')
      | input.eq(b'}');

    JsonCharacterBlock { whitespace, op }
  }

  #[allow(dead_code, reason = "test/reference classifier")]
  pub fn classify_scalar(input: &simd::Simd8x64<u8>) -> JsonCharacterBlock {
    let mut buf = [0; 64];
    input.store(&mut buf);

    let mut whitespace = 0;
    let mut op = 0;
    for (i, b) in buf.into_iter().enumerate() {
      let bit = 1u64 << i;
      match b {
        b' ' | b'\t' | b'\n' | b'\r' => whitespace |= bit,
        b',' | b':' | b'[' | b'{' | b']' | b'}' => op |= bit,
        _ => {}
      }
    }

    JsonCharacterBlock { whitespace, op }
  }

  #[cfg(all(
    feature = "simd",
    target_arch = "aarch64",
    target_feature = "neon"
  ))]
  #[inline(always)]
  // https://github.com/simdjson/simdjson/blob/2887a17bab8ccf8970d3adcf28718a1071e8b836/src/arm64.cpp#L40
  pub fn classify_aarch64_neon(
    input: &simd::Simd8x64<u8>,
  ) -> JsonCharacterBlock {
    use simd::width_128::Simd8;
    use simd::width_128::Simd8x64;
    use simd::width_128::make_u8x16;
    let table1 =
      make_u8x16(16, 0, 0, 0, 0, 0, 0, 0, 0, 8, 12, 1, 2, 9, 0, 0).into();
    let table2 =
      make_u8x16(8, 0, 18, 4, 0, 1, 0, 1, 0, 0, 0, 3, 2, 1, 0, 0).into();

    let v = simd::Simd8x64::from_chunks([
      (input.chunks[0] & Simd8::<u8>::splat(0xf)).lookup_16_table(table1)
        & (input.chunks[0].shr::<4>()).lookup_16_table(table2),
      (input.chunks[1] & Simd8::<u8>::splat(0xf)).lookup_16_table(table1)
        & (input.chunks[1].shr::<4>()).lookup_16_table(table2),
      (input.chunks[2] & Simd8::<u8>::splat(0xf)).lookup_16_table(table1)
        & (input.chunks[2].shr::<4>()).lookup_16_table(table2),
      (input.chunks[3] & Simd8::<u8>::splat(0xf)).lookup_16_table(table1)
        & (input.chunks[3].shr::<4>()).lookup_16_table(table2),
    ]);

    let op = Simd8x64::from_chunks([
      v.chunks[0].any_bits_set(0x7.into()),
      v.chunks[1].any_bits_set(0x7.into()),
      v.chunks[2].any_bits_set(0x7.into()),
      v.chunks[3].any_bits_set(0x7.into()),
    ])
    .to_bitmask();

    let whitespace = Simd8x64::from_chunks([
      v.chunks[0].any_bits_set(0x18.into()),
      v.chunks[1].any_bits_set(0x18.into()),
      v.chunks[2].any_bits_set(0x18.into()),
      v.chunks[3].any_bits_set(0x18.into()),
    ])
    .to_bitmask();

    JsonCharacterBlock { whitespace, op }
  }

  #[cfg(all(feature = "simd", target_arch = "x86_64"))]
  #[target_feature(enable = "ssse3")]
  // https://github.com/simdjson/simdjson/blob/2887a17bab8ccf8970d3adcf28718a1071e8b836/src/icelake.cpp#L48
  pub unsafe fn classify_x86_ssse3(
    input: &simd::width_128::Simd8x64<u8>,
  ) -> JsonCharacterBlock {
    use simd::width_128::Simd8x64;
    use simd::width_128::make_u8x16;

    use crate::simd::width_128::Simd8;
    // These lookups rely on the fact that anything < 127 will match the lower 4 bits, which is why
    // we can't use the generic lookup_16.
    let whitespace_table = make_u8x16(
      b' ', 100, 100, 100, 17, 100, 113, 2, 100, b'\t', b'\n', 112, 100, b'\r',
      100, 100,
    );
    // The 6 operators (:,[]{}) have these values:
    //
    // , 2C
    // : 3A
    // [ 5B
    // { 7B
    // ] 5D
    // } 7D
    //
    // If you use | 0x20 to turn [ and ] into { and }, the lower 4 bits of each character is unique.
    // We exploit this, using a simd 4-bit lookup to tell us which character match against, and then
    // match it (against | 0x20).
    //
    // To prevent recognizing other characters, everything else gets compared with 0, which cannot
    // match due to the | 0x20.
    //
    // NOTE: Due to the | 0x20, this ALSO treats <FF> and <SUB> (control characters 0C and 1A) like ,
    // and :. This gets caught in stage 2, which checks the actual character to ensure the right
    // operators are in the right places.
    let op_table = make_u8x16(
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b':', b'{', // : = 3A, [ = 5B, { = 7B
      b',', b'}', 0, 0, // , = 2C, ] = 5D, } = 7D
    );

    #[inline(always)]
    fn shuffle(table: wide::u8x16, input: Simd8<u8>) -> Simd8<u8> {
      // SAFETY: `classify_x86_ssse3` is compiled with and must be called
      // only when SSSE3 is available. Both operands are 128-bit vector
      // values with the expected layout.
      unsafe {
        std::arch::x86_64::_mm_shuffle_epi8(
          bytemuck::must_cast(table),
          bytemuck::must_cast(input.base),
        )
      }
      .into()
    }

    // We compute whitespace and op separately. If the code later only use one or the
    // other, given the fact that all functions are aggressively inlined, we can
    // hope that useless computations will be omitted. This is namely case when
    // minifying (we only need whitespace).
    let whitespace = input.cmp_eq_mask(&Simd8x64::from_chunks([
      shuffle(whitespace_table, input.chunks[0]),
      shuffle(whitespace_table, input.chunks[1]),
      shuffle(whitespace_table, input.chunks[2]),
      shuffle(whitespace_table, input.chunks[3]),
    ]));

    let curlified = Simd8x64::from_chunks([
      input.chunks[0] | 0x20.into(),
      input.chunks[1] | 0x20.into(),
      input.chunks[2] | 0x20.into(),
      input.chunks[3] | 0x20.into(),
    ]);

    let op = curlified.cmp_eq_mask(&Simd8x64::from_chunks([
      shuffle(op_table, curlified.chunks[0]),
      shuffle(op_table, curlified.chunks[1]),
      shuffle(op_table, curlified.chunks[2]),
      shuffle(op_table, curlified.chunks[3]),
    ]));

    JsonCharacterBlock { whitespace, op }
  }

  #[cfg(all(feature = "simd", target_arch = "x86_64"))]
  fn classify_x86_ssse3_dispatch(
    input: &simd::width_128::Simd8x64<u8>,
  ) -> JsonCharacterBlock {
    // SAFETY: this function is only selected by `runtime_classifier` after
    // checking `is_x86_feature_detected!("ssse3")`.
    unsafe { classify_x86_ssse3(input) }
  }

  #[cfg(all(feature = "simd", target_arch = "x86_64"))]
  pub fn runtime_classifier() -> CharacterClassifier {
    static CLASSIFIER: std::sync::OnceLock<CharacterClassifier> =
      std::sync::OnceLock::new();
    *CLASSIFIER.get_or_init(|| {
      if std::is_x86_feature_detected!("ssse3") {
        classify_x86_ssse3_dispatch
      } else {
        classify_by_comparison
      }
    })
  }
}

impl JsonCharacterBlock {
  #[inline(always)]
  /// Classify a block of JSON text
  pub fn classify(input: &simd::Simd8x64<u8>) -> Self {
    #[cfg(all(
      feature = "simd",
      target_arch = "aarch64",
      target_feature = "neon"
    ))]
    {
      classify::classify_aarch64_neon(input)
    }
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    {
      (classify::runtime_classifier())(input)
    }
    #[cfg(not(any(
      all(feature = "simd", target_arch = "aarch64", target_feature = "neon"),
      all(feature = "simd", target_arch = "x86_64")
    )))]
    {
      classify::classify_by_comparison(input)
    }
  }

  /// Returns a mask of whitespace characters
  pub fn whitespace(&self) -> u64 {
    self.whitespace
  }

  /// Returns a mask of operator characters
  pub fn op(&self) -> u64 {
    self.op
  }

  /// Returns a mask of scalar characters (not whitespace or operators)
  pub fn scalar(&self) -> u64 {
    !(self.op() | self.whitespace())
  }
}

/// Scans JSON for string ranges and structural character candidates.
///
/// String and character classification are intentionally computed separately:
/// the structural mask may include bytes inside strings, and callers combine it
/// with the string mask when deciding which bytes are real tokens.
pub(crate) struct JsonScanner {
  string_scanner: JsonStringScanner,
  #[cfg(all(feature = "simd", target_arch = "x86_64"))]
  character_classifier: CharacterClassifier,
}

impl JsonScanner {
  pub fn new() -> Self {
    Self {
      string_scanner: JsonStringScanner::new(),
      #[cfg(all(feature = "simd", target_arch = "x86_64"))]
      character_classifier: classify::runtime_classifier(),
    }
  }

  #[inline(always)]
  pub fn next(
    &mut self,
    input: &simd::Simd8x64<u8>,
  ) -> (JsonStringBlock, JsonCharacterBlock) {
    let strings = self.string_scanner.next(input);
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    let characters = (self.character_classifier)(input);
    #[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
    let characters = JsonCharacterBlock::classify(input);

    (strings, characters)
  }

  #[inline(always)]
  pub fn finish(&self) -> Result<(), Error> {
    self.string_scanner.finish()
  }
}

pub(crate) struct BitIndexer {
  last_was_quote: bool,
}

#[inline(always)]
fn zero_leading_bit(rev_bits: u64, leading_zeroes: u32) -> u64 {
  rev_bits ^ (0x8000000000000000u64.wrapping_shr(leading_zeroes))
}

impl BitIndexer {
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      last_was_quote: false,
    }
  }

  #[inline(always)]
  pub fn write_index(
    &mut self,
    index: u32,
    rev_bits: &mut u64,
    quotes: u64,
    tail: &mut Vec<Token>,
  ) {
    if *rev_bits == 0 {
      return;
    }
    let lz = rev_bits.leading_zeros();
    let is_quote = quotes & (1u64.wrapping_shl(lz)) != 0;
    if is_quote {
      if self.last_was_quote {
        tail.last_mut().unwrap().set_end(index + lz);
        self.last_was_quote = false;
      } else {
        tail.push(Token::new(
          index + lz + 1,
          index + lz + 1,
          TokenKind::String,
        ));
        self.last_was_quote = true;
      }
    } else {
      tail.push(Token::new(index + lz, index + lz + 1, TokenKind::Operator));
      self.last_was_quote = is_quote;
    }
    *rev_bits = zero_leading_bit(*rev_bits, lz);
  }

  #[inline(always)]
  pub fn write_indexes(
    &mut self,
    index: u32,
    rev_bits: &mut u64,
    quotes: u64,
    tail: &mut Vec<Token>,
  ) {
    self.write_index(index, rev_bits, quotes, tail);
    self.write_index(index, rev_bits, quotes, tail);
    self.write_index(index, rev_bits, quotes, tail);
    self.write_index(index, rev_bits, quotes, tail);
  }

  #[inline(always)]
  #[allow(
    clippy::too_many_arguments,
    reason = "keeps hot token indexing loop allocation-free"
  )]
  pub fn write_indexes_stepped(
    &mut self,
    index: u32,
    rev_bits: &mut u64,
    cnt: usize,
    start: usize,
    end: usize,
    quotes: u64,
    tail: &mut Vec<Token>,
  ) {
    self.write_indexes(index, rev_bits, quotes, tail);
    if start + 4 < end && start + 4 < cnt {
      self.write_indexes(index, rev_bits, quotes, tail);
    }
    if start + 8 < end && start + 8 < cnt {
      self.write_indexes(index, rev_bits, quotes, tail);
    }
    if start + 12 < end && start + 12 < cnt {
      self.write_indexes(index, rev_bits, quotes, tail);
    }
    if start + 16 < end && start + 16 < cnt {
      self.write_indexes(index, rev_bits, quotes, tail);
    }
    if start + 20 < end && start + 20 < cnt {
      self.write_indexes(index, rev_bits, quotes, tail);
    }
  }

  #[inline(always)]
  pub fn write(
    &mut self,
    index: u32,
    bits: u64,
    quotes: u64,
    tail: &mut Vec<Token>,
  ) {
    if bits == 0 {
      return;
    }

    let cnt = bits.count_ones();
    let mut rev_bits = bits.reverse_bits();

    self.write_indexes_stepped(
      index,
      &mut rev_bits,
      cnt as usize,
      0,
      24,
      quotes,
      tail,
    );

    if cnt > 24 {
      for _ in 24..cnt {
        self.write_index(index, &mut rev_bits, quotes, tail);
      }
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Token {
  data: u64,
}

impl Token {
  pub fn new(start: u32, end: u32, kind: TokenKind) -> Self {
    Self {
      data: (start as u64)
        | ((end as u64 & 0x7FFFFFFFFFFFFFFF) << 32)
        | ((kind as u64) << 63),
    }
  }

  pub fn start(self) -> u32 {
    (self.data & 0xFFFFFFFF) as u32
  }
  pub fn end(self) -> u32 {
    ((self.data & 0x7FFFFFFFFFFFFFFF) >> 32) as u32
  }

  pub fn set_end(&mut self, end: u32) {
    self.data = (self.data & 0x80000000_FFFFFFFF)
      | (((end as u64) & 0x7FFFFFFFFFFFFFFF) << 32);
  }

  pub fn kind(self) -> TokenKind {
    match self.data >> 63 {
      0 => TokenKind::Operator,
      1 => TokenKind::String,
      _ => unreachable!(),
    }
  }

  pub fn value<'a>(&self, input: &'a [u8]) -> &'a [u8] {
    &input[self.start() as usize..self.end() as usize]
  }

  pub fn string_value<'a>(&self, input: &'a str) -> &'a str {
    debug_assert_eq!(self.kind(), TokenKind::String);
    let bytes = self.value(input.as_bytes());
    // SAFETY: `input` is valid UTF-8, and string token boundaries are
    // quote-delimited byte positions from that same string. Quotes are ASCII
    // single-byte delimiters, so slicing just inside them preserves UTF-8
    // character boundaries.
    unsafe { std::str::from_utf8_unchecked(bytes) }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenKind {
  Operator = 0,
  String = 1,
}
pub(crate) struct Tokenizer<'a, Mask: OpMask = NoCommaOrColon> {
  scanner: JsonScanner,
  tokens: Vec<Token>,
  block_reader: BufBlockReader<'a, 64>,
  idx: u32,

  bit_indexer: BitIndexer,

  _marker: std::marker::PhantomData<Mask>,
}

pub trait CharsEq {
  fn eq_mask(&self, value: u8) -> u64;
}

impl CharsEq for simd::Simd8x64<u8> {
  #[inline(always)]
  fn eq_mask(&self, value: u8) -> u64 {
    self.eq(value)
  }
}
pub trait OpMask {
  fn op_mask(b: &impl CharsEq) -> u64;
}

pub struct NoCommaOrColon;

impl OpMask for NoCommaOrColon {
  #[inline(always)]
  fn op_mask(b: &impl CharsEq) -> u64 {
    b.eq_mask(b',') | b.eq_mask(b':')
  }
}

impl<'a, Mask: OpMask> Tokenizer<'a, Mask> {
  pub fn new(input: &'a [u8]) -> Result<Self, Error> {
    if input.len() > 0x7FFF_FFFF {
      return Err(Error::InputTooLong);
    }
    Ok(Self {
      scanner: JsonScanner::new(),
      tokens: Vec::with_capacity(input.len() / 4),
      block_reader: BufBlockReader::new(input),
      idx: 0,
      _marker: std::marker::PhantomData,
      bit_indexer: BitIndexer::new(),
    })
  }

  #[inline(always)]
  fn process_json_block(
    &mut self,
    strings: JsonStringBlock,
    characters: JsonCharacterBlock,
    dont_care: u64,
  ) {
    let ops = characters.op() & !strings.in_string & !dont_care;
    let quotes = strings.quote;
    self
      .bit_indexer
      .write(self.idx, ops | quotes, quotes, &mut self.tokens);
  }

  pub fn tokenize(mut self) -> Result<Vec<Token>, Error> {
    while self.block_reader.has_full_block() {
      let block = self.block_reader.full_block();
      let block =
        simd::Simd8x64::<u8>::load(arrayref::array_ref![block, 0, 64]);
      let (strings, characters) = self.scanner.next(&block);
      self.block_reader.advance();
      let dont_care = Mask::op_mask(&block);
      self.process_json_block(strings, characters, dont_care);
      self.idx += 64;
    }

    let mut remainder_buf = [0; 64];
    let _pad = self.block_reader.get_remainder(&mut remainder_buf);
    let block =
      simd::Simd8x64::<u8>::load(arrayref::array_ref![&remainder_buf, 0, 64]);
    let (strings, characters) = self.scanner.next(&block);
    self.block_reader.advance();
    let dont_care = Mask::op_mask(&block);
    self.process_json_block(strings, characters, dont_care);
    self.idx += 64;
    self.scanner.finish()?;
    Ok(self.tokens)
  }
}

fn structural_operator(input: &[u8], token: Token) -> Option<u8> {
  let byte = input[token.start() as usize];
  matches!(byte, b'{' | b'}' | b'[' | b']').then_some(byte)
}

pub(crate) fn pluck_versions_from_tokens(
  input: &str,
  tokens: Vec<Token>,
) -> Result<Versions<'_>, Error> {
  enum State<'i> {
    Start,
    InVersions,
    WantVersion,
    InDistTags,
    WantDistTagValue(&'i str),
  }
  let mut state = State::Start;
  let mut versions = Vec::new();

  let mut version_ranges = Vec::new();

  let mut dist_tags = rustc_hash::FxHashMap::<&str, &str>::default();
  let mut object_depth = 0;
  let mut finished_early = false;
  let input_bytes = input.as_bytes();
  for token in tokens {
    match token.kind() {
      TokenKind::String => {
        if object_depth == 1 {
          let v: &[u8] = token.value(input_bytes);
          if v == b"versions" {
            state = State::InVersions;
          } else if v == b"dist-tags" {
            state = State::InDistTags;
          }
        } else if object_depth == 2 && matches!(state, State::InVersions) {
          versions.push(token.string_value(input));
          state = State::WantVersion;
        } else if object_depth == 2 && matches!(state, State::InDistTags) {
          let key = token.string_value(input);
          dist_tags.insert(key, "");
          state = State::WantDistTagValue(key);
        } else if object_depth == 2
          && let State::WantDistTagValue(key) = state
        {
          let dist_tag = dist_tags.get_mut(key);
          if let Some(dist_tag) = dist_tag {
            *dist_tag = token.string_value(input);
          }
          state = State::InDistTags;
        }
      }
      TokenKind::Operator => {
        let Some(v) = structural_operator(input_bytes, token) else {
          continue;
        };
        if v == b'{' {
          object_depth += 1;
          if object_depth == 3 && matches!(state, State::WantVersion) {
            version_ranges.push((token.start(), token.end()));
          }
        } else if v == b'}' {
          if object_depth == 2
            && matches!(state, State::InVersions | State::InDistTags)
          {
            state = State::Start;
            if !dist_tags.is_empty() && !versions.is_empty() {
              finished_early = true;
              break;
            }
          } else if object_depth == 3 && matches!(state, State::WantVersion) {
            if let Some(last) = version_ranges.last_mut() {
              last.1 = token.end();
            }
            state = State::InVersions;
          }
          if object_depth == 0 {
            return Err(Error::UnmatchedBrace(token.start() as usize));
          }
          object_depth -= 1;
        } else if v == b'[' {
          object_depth += 1;
        } else if v == b']' {
          if object_depth == 0 {
            return Err(Error::UnmatchedBrace(token.start() as usize));
          }
          object_depth -= 1;
        }
      }
    }
  }
  if !finished_early && object_depth != 0 {
    return Err(Error::UnmatchedBrace(input.len()));
  }
  Ok(Versions {
    versions,
    version_ranges,
    dist_tags,
  })
}

#[derive(Debug, Clone)]
pub struct Versions<'i> {
  pub versions: Vec<&'i str>,
  pub version_ranges: Vec<(u32, u32)>,
  pub dist_tags: rustc_hash::FxHashMap<&'i str, &'i str>,
}

pub fn pluck_versions(input: &str) -> Result<Versions<'_>, Error> {
  let tokenizer = Tokenizer::<NoCommaOrColon>::new(input.as_bytes())?;
  let tokens = tokenizer.tokenize()?;
  pluck_versions_from_tokens(input, tokens)
}

pub(crate) fn pluck_packument_index_from_tokens(
  input: &str,
  tokens: Vec<Token>,
) -> Result<PackumentIndex<'_>, Error> {
  enum State<'i> {
    Start,
    WantNameValue,
    WantDenoEtagValue,
    InVersions,
    WantVersion(&'i str),
    InDistTags,
    WantDistTagValue(&'i str),
    InTime,
    WantTimeValue(&'i str),
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum ObjectKind {
    Unknown,
    Version,
    Dist,
    NpmUser,
    Attestations,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum PendingObject {
    Dist,
    NpmUser,
    Attestations,
  }

  #[derive(Debug, Default)]
  struct TrustSignals {
    has_provenance: bool,
    has_trusted_publisher: bool,
    has_approver: bool,
  }

  impl TrustSignals {
    fn evidence(&self) -> TrustEvidence {
      if self.has_approver {
        TrustEvidence::StagedPublish
      } else if self.has_trusted_publisher && self.has_provenance {
        TrustEvidence::TrustedPublisher
      } else {
        TrustEvidence::Provenance
      }
    }
  }

  let mut state = State::Start;
  let mut name = None;
  let mut deno_etag = None;
  let mut versions = Vec::new();
  let mut version_ranges = Vec::new();
  let mut dist_tags = rustc_hash::FxHashMap::<&str, &str>::default();
  let mut time = rustc_hash::FxHashMap::<&str, &str>::default();
  let mut trust_signals =
    rustc_hash::FxHashMap::<&str, TrustSignals>::default();
  let mut object_depth = 0usize;
  let mut current_version = None;
  let mut object_kinds = Vec::new();
  let mut pending_object = None;
  let input_bytes = input.as_bytes();

  for token in tokens {
    match token.kind() {
      TokenKind::String => {
        let v = token.value(input_bytes);
        if object_depth == 1 {
          if matches!(state, State::WantNameValue) {
            name = Some(token.string_value(input));
            state = State::Start;
          } else if matches!(state, State::WantDenoEtagValue) {
            deno_etag = Some(token.string_value(input));
            state = State::Start;
          } else if v == b"name" {
            state = State::WantNameValue;
          } else if v == b"_deno.etag" {
            state = State::WantDenoEtagValue;
          } else if v == b"versions" {
            state = State::InVersions;
          } else if v == b"dist-tags" {
            state = State::InDistTags;
          } else if v == b"time" {
            state = State::InTime;
          }
        } else if object_depth == 2 && matches!(state, State::InVersions) {
          let version = token.string_value(input);
          versions.push(version);
          state = State::WantVersion(version);
        } else if object_depth == 2 && matches!(state, State::InDistTags) {
          let key = token.string_value(input);
          dist_tags.insert(key, "");
          state = State::WantDistTagValue(key);
        } else if object_depth == 2 && matches!(state, State::InTime) {
          let key = token.string_value(input);
          time.insert(key, "");
          state = State::WantTimeValue(key);
        } else if object_depth == 2 {
          match state {
            State::WantDistTagValue(key) => {
              if let Some(dist_tag) = dist_tags.get_mut(key) {
                *dist_tag = token.string_value(input);
              }
              state = State::InDistTags;
            }
            State::WantTimeValue(key) => {
              if let Some(time_value) = time.get_mut(key) {
                *time_value = token.string_value(input);
              }
              state = State::InTime;
            }
            _ => {}
          }
        } else if let Some(version) = current_version {
          match object_kinds.last().copied().unwrap_or(ObjectKind::Unknown) {
            ObjectKind::Version => {
              if v == b"dist" {
                pending_object = Some(PendingObject::Dist);
              } else if v == b"_npmUser" {
                pending_object = Some(PendingObject::NpmUser);
              } else {
                pending_object = None;
              }
            }
            ObjectKind::Dist => {
              if v == b"attestations" {
                pending_object = Some(PendingObject::Attestations);
              } else {
                pending_object = None;
              }
            }
            ObjectKind::NpmUser => {
              if v == b"approver" {
                trust_signals.entry(version).or_default().has_approver = true;
              } else if v == b"trustedPublisher" {
                trust_signals
                  .entry(version)
                  .or_default()
                  .has_trusted_publisher = true;
              }
            }
            ObjectKind::Attestations => {
              if v == b"provenance" {
                trust_signals.entry(version).or_default().has_provenance = true;
              }
            }
            ObjectKind::Unknown => {}
          }
        }
      }
      TokenKind::Operator => {
        let Some(v) = structural_operator(input_bytes, token) else {
          continue;
        };
        if v == b'{' {
          object_depth += 1;
          let kind = if object_depth == 3 {
            if let State::WantVersion(version) = state {
              version_ranges.push((token.start(), token.end()));
              current_version = Some(version);
              ObjectKind::Version
            } else {
              ObjectKind::Unknown
            }
          } else {
            match (object_kinds.last().copied(), pending_object.take()) {
              (Some(ObjectKind::Version), Some(PendingObject::Dist)) => {
                ObjectKind::Dist
              }
              (Some(ObjectKind::Version), Some(PendingObject::NpmUser)) => {
                ObjectKind::NpmUser
              }
              (Some(ObjectKind::Dist), Some(PendingObject::Attestations)) => {
                ObjectKind::Attestations
              }
              _ => ObjectKind::Unknown,
            }
          };
          object_kinds.push(kind);
        } else if v == b'}' {
          if object_depth == 2
            && matches!(
              state,
              State::InVersions | State::InDistTags | State::InTime
            )
          {
            state = State::Start;
          } else if object_depth == 3 && matches!(state, State::WantVersion(_))
          {
            if let Some(last) = version_ranges.last_mut() {
              last.1 = token.end();
            }
            state = State::InVersions;
            current_version = None;
            pending_object = None;
          }
          if object_depth == 0 {
            return Err(Error::UnmatchedBrace(token.start() as usize));
          }
          object_depth -= 1;
          object_kinds.pop();
        } else if v == b'[' {
          object_depth += 1;
          object_kinds.push(ObjectKind::Unknown);
        } else if v == b']' {
          if object_depth == 0 {
            return Err(Error::UnmatchedBrace(token.start() as usize));
          }
          object_depth -= 1;
          object_kinds.pop();
        }
      }
    }
  }

  if object_depth != 0 {
    return Err(Error::UnmatchedBrace(input.len()));
  }

  let trust_evidence = trust_signals
    .into_iter()
    .filter(|(_, signals)| signals.has_approver || signals.has_provenance)
    .map(|(version, signals)| (version, signals.evidence()))
    .collect();

  Ok(PackumentIndex {
    name,
    deno_etag,
    versions,
    version_ranges,
    dist_tags,
    time,
    trust_evidence,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrustEvidence {
  Provenance,
  TrustedPublisher,
  StagedPublish,
}

#[derive(Debug, Clone)]
pub struct PackumentIndex<'i> {
  pub name: Option<&'i str>,
  pub deno_etag: Option<&'i str>,
  pub versions: Vec<&'i str>,
  pub version_ranges: Vec<(u32, u32)>,
  pub dist_tags: rustc_hash::FxHashMap<&'i str, &'i str>,
  pub time: rustc_hash::FxHashMap<&'i str, &'i str>,
  pub trust_evidence: rustc_hash::FxHashMap<&'i str, TrustEvidence>,
}

pub fn pluck_packument_index(input: &str) -> Result<PackumentIndex<'_>, Error> {
  let tokenizer = Tokenizer::<NoCommaOrColon>::new(input.as_bytes())?;
  let tokens = tokenizer.tokenize()?;
  pluck_packument_index_from_tokens(input, tokens)
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  #[test]
  fn zero_leading_bit_works() {
    let cases: &[(u64, u64)] = &[(0b0100, 0), (0b0101, 0b0001)];
    for (rev_bits, expected) in cases {
      let (rev_bits, expected) = (*rev_bits, *expected);
      let leading_zeroes = rev_bits.leading_zeros();
      let result = zero_leading_bit(rev_bits, leading_zeroes);
      assert_eq!(result, expected);
    }
  }

  fn string(start: u32, s: &str) -> Token {
    Token::new(start, start + s.len() as u32, TokenKind::String)
  }

  fn op(start: u32) -> Token {
    Token::new(start, start + 1, TokenKind::Operator)
  }

  struct TokensBuilder {
    tokens: Vec<Token>,
  }

  impl TokensBuilder {
    fn new() -> Self {
      Self { tokens: Vec::new() }
    }

    fn then(self, offset: u32, f: impl FnOnce(Self, u32) -> Self) -> Self {
      let last_end = self.tokens.last().map_or(0, |t| t.end());
      f(self, last_end + offset)
    }

    fn with_string(mut self, start: u32, s: &str) -> Self {
      self.tokens.push(string(start, s));
      self
    }

    fn with_op(mut self, start: u32) -> Self {
      self.tokens.push(op(start));
      self
    }

    fn string(self, offset: u32, s: &str) -> Self {
      self.then(offset, |b, i| b.with_string(i, s))
    }

    fn op(self, offset: u32) -> Self {
      self.then(offset, |b, i| b.with_op(i))
    }

    fn build(self) -> Vec<Token> {
      self.tokens
    }
  }

  struct KeepAll;

  impl OpMask for KeepAll {
    fn op_mask(_b: &impl CharsEq) -> u64 {
      0
    }
  }

  fn assert_tokens_eq(input: &str, expected: Vec<Token>) {
    let tokens = Tokenizer::<KeepAll>::new(input.as_bytes())
      .unwrap()
      .tokenize()
      .unwrap();
    assert_eq!(tokens, expected);
  }

  fn assert_packument_projection_matches_serde(input: &str) {
    let index = pluck_packument_index(input).unwrap();
    let value: serde_json::Value = serde_json::from_str(input).unwrap();
    let object = value.as_object().unwrap();

    assert_eq!(index.name, object.get("name").and_then(|v| v.as_str()));

    let versions = object["versions"].as_object().unwrap();
    assert_eq!(index.versions.len(), versions.len());
    assert_eq!(index.version_ranges.len(), versions.len());

    for (version, range) in index.versions.iter().zip(&index.version_ranges) {
      assert!(
        versions.contains_key(*version),
        "indexed unknown version {version}"
      );
      let range_json: serde_json::Value =
        serde_json::from_str(&input[range.0 as usize..range.1 as usize])
          .unwrap();
      assert_eq!(range_json, versions[*version]);
    }

    let expected_dist_tags = object
      .get("dist-tags")
      .and_then(|v| v.as_object())
      .into_iter()
      .flatten()
      .filter_map(|(key, value)| {
        value.as_str().map(|value| (key.as_str(), value))
      })
      .collect::<rustc_hash::FxHashMap<_, _>>();
    assert_eq!(index.dist_tags, expected_dist_tags);

    let expected_time = object
      .get("time")
      .and_then(|v| v.as_object())
      .into_iter()
      .flatten()
      .filter_map(|(key, value)| {
        value.as_str().map(|value| (key.as_str(), value))
      })
      .collect::<rustc_hash::FxHashMap<_, _>>();
    assert_eq!(index.time, expected_time);
  }

  fn generated_packument(seed: u32) -> String {
    let version_count = 1 + seed as usize % 8;
    let mut input = String::new();
    input.push_str("{\"_rev\":\"");
    input.push_str(&seed.to_string());
    input.push_str("\",\"name\":\"@scope/pkg-");
    input.push_str(&seed.to_string());
    input
      .push_str("\",\"noise\":{\"versions\":{\"9.9.9\":{}}},\"dist-tags\":{");
    input.push_str("\"latest\":\"1.0.0\"");
    if version_count > 1 {
      input.push_str(",\"beta\":\"1.0.1\"");
    }
    input.push_str("},\"versions\":{");
    for i in 0..version_count {
      if i > 0 {
        input.push(',');
      }
      let version = format!("1.0.{i}");
      input.push('"');
      input.push_str(&version);
      input.push_str("\":{\"version\":\"");
      input.push_str(&version);
      input.push_str("\",\"description\":\"line \\\\n quote \\\" ok\",");
      input.push_str("\"dependencies\":{\"dep\":\"^");
      input.push_str(&i.to_string());
      input.push_str(".0.0\"},");
      if i % 3 == 0 {
        input.push_str("\"dist\":{\"tarball\":\"https://registry.example/pkg.tgz\",\"attestations\":{\"provenance\":true}},");
      }
      if i % 3 == 1 {
        input.push_str("\"dist\":{\"tarball\":\"https://registry.example/pkg.tgz\",\"attestations\":{\"provenance\":true}},");
        input.push_str(
          "\"_npmUser\":{\"trustedPublisher\":{\"id\":\"publisher\"}},",
        );
      }
      if i % 3 == 2 {
        input.push_str("\"_npmUser\":{\"approver\":{\"name\":\"approver\"}},");
      }
      input.push_str("\"nested\":{\"dist\":{\"attestations\":{\"provenance\":true}},\"_npmUser\":{\"approver\":true}}}");
    }
    input.push_str("},\"time\":{\"created\":\"2024-01-01T00:00:00.000Z\"");
    for i in 0..version_count {
      input.push_str(",\"1.0.");
      input.push_str(&i.to_string());
      input.push_str("\":\"2024-01-");
      input.push_str(&format!("{:02}", i + 2));
      input.push_str("T00:00:00.000Z\"");
    }
    input.push_str("}}");
    input
  }

  #[test]
  fn active_classifier_matches_scalar_classifier() {
    let mut input = [0u8; 64];
    let sample = br#" { "x": [1, 2, {"y": "\n"}] }	"#;
    input[..sample.len()].copy_from_slice(sample);
    input[sample.len()] = b'\r';
    input[40] = 0x0c;
    input[41] = 0x1a;
    input[42] = 0xff;

    let block = simd::Simd8x64::<u8>::load(&input);
    let active = JsonCharacterBlock::classify(&block);
    let comparison = classify::classify_by_comparison(&block);
    let scalar = classify::classify_scalar(&block);

    assert_eq!(active.whitespace(), comparison.whitespace());
    assert_eq!(active.op(), comparison.op());
    assert_eq!(active.whitespace(), scalar.whitespace());
    assert_eq!(active.op(), scalar.op());
  }

  #[test]
  fn incomplete_string_works() {
    let input = r#"{"versions":{"aaaaaaaaaaaaaaaaaaaaaaaaaaaaa":{},"bcdefghijkabcd":"asdf"}}"#;

    let expected = TokensBuilder::new()
      .op(0)
      .string(1, "versions")
      .op(1)
      .op(0)
      .string(1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
      .op(1) // :
      .op(0) // {
      .op(0) // }
      .op(0) // ,
      .string(1, "bcdefghijkabcd")
      .op(1) // :
      .string(1, "asdf")
      .op(1) // }
      .op(0) // }
      .build();
    assert_tokens_eq(input, expected);

    let input = r#"{"versions":{"aaaaaaaaaaaaaaaaaaaaaaaaaaaaa":{},"bcdefghijkab":"asdf"}}"#;
    let expected = TokensBuilder::new()
      .op(0)
      .string(1, "versions")
      .op(1) // :
      .op(0) // {
      .string(1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
      .op(1) // :
      .op(0) // {
      .op(0) // }
      .op(0) // ,
      .string(1, "bcdefghijkab")
      .op(1) // :
      .string(1, "asdf")
      .op(1) // }
      .op(0) // }
      .build();
    assert_tokens_eq(input, expected);
  }

  #[test]
  fn split_utf8_works() {
    let input = r#"{"versions":{"aaaaaaaaaaaaaaaaaaaaaaaaaaaaa":{},"bcdefghijkabc♥♥":{}}}"#;
    let builder = TokensBuilder::new();
    let expected = builder
      .op(0) // {
      .string(1, "versions")
      .op(1) // :
      .op(0) // {
      .string(1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
      .op(1) // :
      .op(0) // {
      .op(0) // }
      .op(0) // ,
      .string(1, "bcdefghijkabc♥♥")
      .op(1) // :
      .op(0) // {
      .op(0) // }
      .op(0) // }
      .op(0) // }
      .build();
    assert_tokens_eq(input, expected);
  }

  #[test]
  fn test_pluck_versions() {
    let input = r#"{"versions":{"aaaaaaaaaaaaaaaaaaaaaaaaaaaaa":{},"bcdefghijkabc♥♥":{}},"dist-tags":{"latest":"foo","bar":"baz"}}"#;
    let versions = pluck_versions(input).unwrap();
    assert_eq!(
      versions.versions,
      vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "bcdefghijkabc♥♥"]
    );
    assert_eq!(
      versions.dist_tags,
      vec![("latest", "foo"), ("bar", "baz")]
        .into_iter()
        .collect()
    );
  }

  #[test]
  fn test_pluck_packument_index() {
    let input = r#"{"name":"pkg","_deno.etag":"etag-1","dist-tags":{"latest":"1.1.0"},"versions":{"1.0.0":{"version":"1.0.0","dist":{"attestations":{"provenance":{"x":1}}}},"1.1.0":{"version":"1.1.0","_npmUser":{"trustedPublisher":{"x":1},"approver":{"name":"a"}}}},"time":{"created":"2024-01-01T00:00:00.000Z","modified":"2024-01-03T00:00:00.000Z","1.0.0":"2024-01-02T00:00:00.000Z","1.1.0":"2024-01-03T00:00:00.000Z"}}"#;
    let index = pluck_packument_index(input).unwrap();
    assert_eq!(index.name, Some("pkg"));
    assert_eq!(index.deno_etag, Some("etag-1"));
    assert_eq!(index.versions, vec!["1.0.0", "1.1.0"]);
    assert_eq!(
      index.dist_tags,
      vec![("latest", "1.1.0")].into_iter().collect()
    );
    assert_eq!(
      index.time,
      vec![
        ("created", "2024-01-01T00:00:00.000Z"),
        ("modified", "2024-01-03T00:00:00.000Z"),
        ("1.0.0", "2024-01-02T00:00:00.000Z"),
        ("1.1.0", "2024-01-03T00:00:00.000Z"),
      ]
      .into_iter()
      .collect()
    );
    assert_eq!(
      index.trust_evidence,
      vec![
        ("1.0.0", TrustEvidence::Provenance),
        ("1.1.0", TrustEvidence::StagedPublish),
      ]
      .into_iter()
      .collect()
    );

    let first_range = index.version_ranges[0];
    assert_eq!(
      &input[first_range.0 as usize..first_range.1 as usize],
      r#"{"version":"1.0.0","dist":{"attestations":{"provenance":{"x":1}}}}"#
    );
  }

  #[test]
  fn packument_index_ignores_nested_deno_etag() {
    let input = r#"{"name":"pkg","versions":{"1.0.0":{"version":"1.0.0","_deno.etag":"nested"}}}"#;
    let index = pluck_packument_index(input).unwrap();
    assert_eq!(index.name, Some("pkg"));
    assert_eq!(index.deno_etag, None);
    assert_eq!(index.versions, vec!["1.0.0"]);
  }

  #[test]
  fn packument_index_matches_serde_json_projection() {
    let input = r#"{
          "noise": {"versions": {"ignored": {}}},
          "name": "pkg",
          "dist-tags": {"latest": "2.0.0", "beta": "2.1.0-beta.0"},
          "versions": {
            "1.0.0": {
              "version": "1.0.0",
              "dist": {"attestations": {"provenance": true}},
              "nested": {"_npmUser": {"approver": true}}
            },
            "2.0.0": {
              "version": "2.0.0",
              "_npmUser": {"trustedPublisher": {"id": "pub"}},
              "dist": {"attestations": {"provenance": true}}
            },
            "2.1.0-beta.0": {
              "version": "2.1.0-beta.0",
              "_npmUser": {"approver": {"name": "approver"}}
            }
          },
          "time": {
            "created": "2024-01-01T00:00:00.000Z",
            "1.0.0": "2024-01-02T00:00:00.000Z",
            "2.0.0": "2024-01-03T00:00:00.000Z",
            "2.1.0-beta.0": "2024-01-04T00:00:00.000Z"
          }
        }"#;

    let index = pluck_packument_index(input).unwrap();
    let value: serde_json::Value = serde_json::from_str(input).unwrap();
    let object = value.as_object().unwrap();

    assert_eq!(index.name, object.get("name").and_then(|v| v.as_str()));

    let expected_versions = object["versions"]
      .as_object()
      .unwrap()
      .keys()
      .map(String::as_str)
      .collect::<Vec<_>>();
    assert_eq!(index.versions, expected_versions);

    for version in &index.versions {
      let range = index.version_ranges[index
        .versions
        .iter()
        .position(|candidate| candidate == version)
        .unwrap()];
      let range_json: serde_json::Value =
        serde_json::from_str(&input[range.0 as usize..range.1 as usize])
          .unwrap();
      assert_eq!(range_json, object["versions"][*version]);
    }

    let expected_dist_tags = object["dist-tags"]
      .as_object()
      .unwrap()
      .iter()
      .map(|(key, value)| (key.as_str(), value.as_str().unwrap()))
      .collect::<rustc_hash::FxHashMap<_, _>>();
    assert_eq!(index.dist_tags, expected_dist_tags);

    let expected_time = object["time"]
      .as_object()
      .unwrap()
      .iter()
      .map(|(key, value)| (key.as_str(), value.as_str().unwrap()))
      .collect::<rustc_hash::FxHashMap<_, _>>();
    assert_eq!(index.time, expected_time);

    assert_eq!(
      index.trust_evidence,
      vec![
        ("1.0.0", TrustEvidence::Provenance),
        ("2.0.0", TrustEvidence::TrustedPublisher),
        ("2.1.0-beta.0", TrustEvidence::StagedPublish),
      ]
      .into_iter()
      .collect()
    );
  }

  #[test]
  fn packument_index_handles_top_level_fields_in_any_order() {
    let input = r#"{
          "time": {
            "1.0.0": "2024-01-02T00:00:00.000Z",
            "created": "2024-01-01T00:00:00.000Z"
          },
          "metadata": {
            "versions": {"ignored": {}},
            "time": {"ignored": "2020-01-01T00:00:00.000Z"},
            "dist-tags": {"ignored": "0.0.0"}
          },
          "versions": {
            "1.0.0": {
              "version": "1.0.0",
              "files": [
                {"_npmUser": {"approver": true}},
                {"dist": {"attestations": {"provenance": true}}}
              ],
              "dist": {"attestations": {"provenance": true}}
            }
          },
          "name": "pkg",
          "dist-tags": {"latest": "1.0.0"}
        }"#;

    assert_packument_projection_matches_serde(input);
    let index = pluck_packument_index(input).unwrap();
    assert_eq!(
      index.trust_evidence,
      vec![("1.0.0", TrustEvidence::Provenance)]
        .into_iter()
        .collect()
    );
  }

  #[test]
  fn packument_index_ignores_trust_evidence_outside_direct_paths() {
    let input = r#"{
          "name": "pkg",
          "versions": {
            "1.0.0": {
              "version": "1.0.0",
              "nested": {
                "dist": {"attestations": {"provenance": true}},
                "_npmUser": {
                  "trustedPublisher": {"id": "publisher"},
                  "approver": {"name": "approver"}
                }
              }
            },
            "1.0.1": {
              "version": "1.0.1",
              "dist": {
                "nested": {"attestations": {"provenance": true}}
              },
              "_npmUser": {
                "nested": {
                  "trustedPublisher": {"id": "publisher"},
                  "approver": {"name": "approver"}
                }
              }
            }
          },
          "dist-tags": {"latest": "1.0.1"},
          "time": {
            "1.0.0": "2024-01-02T00:00:00.000Z",
            "1.0.1": "2024-01-03T00:00:00.000Z"
          }
        }"#;

    assert_packument_projection_matches_serde(input);
    let index = pluck_packument_index(input).unwrap();
    assert!(index.trust_evidence.is_empty());
  }

  #[test]
  fn packument_index_requires_provenance_for_trusted_publisher() {
    let input = r#"{
          "name": "pkg",
          "versions": {
            "1.0.0": {
              "version": "1.0.0",
              "_npmUser": {"trustedPublisher": {"id": "publisher"}}
            },
            "1.0.1": {
              "version": "1.0.1",
              "_npmUser": {"trustedPublisher": {"id": "publisher"}},
              "dist": {"attestations": {"provenance": true}}
            },
            "1.0.2": {
              "version": "1.0.2",
              "_npmUser": {"approver": {"name": "approver"}}
            }
          }
        }"#;

    let index = pluck_packument_index(input).unwrap();
    assert_eq!(
      index.trust_evidence,
      vec![
        ("1.0.1", TrustEvidence::TrustedPublisher),
        ("1.0.2", TrustEvidence::StagedPublish),
      ]
      .into_iter()
      .collect()
    );
  }

  #[test]
  fn generated_packuments_match_serde_json_projection() {
    for seed in 0..64 {
      let input = generated_packument(seed);
      assert_packument_projection_matches_serde(&input);

      let index = pluck_packument_index(&input).unwrap();
      let expected_trust_evidence = index
        .versions
        .iter()
        .enumerate()
        .map(|(i, version)| {
          let evidence = match i % 3 {
            0 => TrustEvidence::Provenance,
            1 => TrustEvidence::TrustedPublisher,
            _ => TrustEvidence::StagedPublish,
          };
          (*version, evidence)
        })
        .collect::<rustc_hash::FxHashMap<_, _>>();
      assert_eq!(index.trust_evidence, expected_trust_evidence);
    }
  }

  #[test]
  fn escaped_quotes() {
    let input = r#"{"versions":{"aaa\"bbb":{}}}"#;
    let expected = TokensBuilder::new()
      .op(0)
      .string(1, "versions")
      .op(1)
      .op(0)
      .string(1, r#"aaa\"bbb"#)
      .op(1)
      .op(0)
      .op(0)
      .op(0)
      .op(0)
      .build();
    assert_tokens_eq(input, expected);
  }

  #[test]
  fn small_input() {
    let input = r#"{"foo":"bar"}"#;
    let expected = TokensBuilder::new()
      .op(0)
      .string(1, "foo")
      .op(1)
      .string(1, "bar")
      .op(1)
      .build();
    assert_tokens_eq(input, expected);
  }

  #[test]
  fn invalid_input() {
    let input = r#"{"versions":{}}}"#;
    let _versions = pluck_versions(input);
  }

  #[test]
  fn unclosed_string() {
    let input = r#"{"versions:{}}"#;
    let error = pluck_versions(input).unwrap_err();
    assert_eq!(error, Error::UnclosedString);
  }

  #[test]
  fn unclosed_object() {
    let input = r#"{"versions":{"1.0.0":{}"#;
    let error = pluck_versions(input).unwrap_err();
    assert_eq!(error, Error::UnmatchedBrace(input.len()));

    let error = pluck_packument_index(input).unwrap_err();
    assert_eq!(error, Error::UnmatchedBrace(input.len()));
  }

  #[test]
  fn malformed_inputs_do_not_panic() {
    let inputs = [
      "",
      "{",
      "}",
      "[",
      "]",
      "{{{{",
      "}}}}",
      r#"{"versions":["1.0.0"]}"#,
      r#"{"versions":{"1.0.0":null}}"#,
      r#"{"versions":{"1.0.0":[]}}"#,
      r#"{"versions":{"1.0.0":{"version":"1.0.0"}"#,
      r#"{"dist-tags":{"latest":null},"versions":{}}"#,
      r#"{"time":{"1.0.0":null},"versions":{}}"#,
      r#"{"name":null,"versions":{}}"#,
      r#"{"versions":{"\"":{}},"dist-tags":{"latest":"\""}}"#,
    ];

    for input in inputs {
      let _ = pluck_versions(input);
      let _ = pluck_packument_index(input);
    }
  }

  #[test]
  fn false_positive_operator_candidates_do_not_panic() {
    let input = std::str::from_utf8(&[
      123, 34, 118, 101, 114, 115, 105, 111, 110, 115, 34, 58, 123, 34, 49, 46,
      48, 46, 48, 34, 58, 123, 34, 118, 101, 114, 115, 105, 111, 110, 34, 58,
      34, 49, 46, 48, 34, 34, 34, 34, 34, 34, 35, 34, 34, 34, 34, 105, 115,
      101, 34, 205, 132, 221, 137, 101, 114, 115, 105, 111, 110, 34, 34, 34,
      34, 34, 34, 34, 34, 34, 42, 34, 34, 34, 34, 34, 34, 34, 34, 34, 92, 0, 0,
      0, 34, 34, 34, 34, 34, 34, 92, 0, 0, 0, 34, 34, 34, 34, 34, 34, 34, 34,
      34, 34, 34, 34, 34, 34, 34, 34, 34, 34, 34, 34, 34, 34, 34, 48, 34, 58,
      34, 50, 48, 50, 52, 45, 48, 49, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 115, 34, 58, 123, 34, 57, 46, 39, 57, 46, 57, 34, 58, 123,
      125, 125, 45, 34, 100, 105, 115, 116, 45, 116, 97, 46, 48, 34, 125,
    ])
    .unwrap();

    let _ = pluck_versions(input);
    let _ = pluck_packument_index(input);
  }
}
