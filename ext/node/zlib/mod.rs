use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use libz_sys::*;
use std::borrow::Cow;
use std::cell::RefCell;

#[inline]
fn check(condition: bool, msg: &str) -> Result<(), AnyError> {
  if condition {
    Ok(())
  } else {
    Err(type_error(msg.to_string()))
  }
}

const Z_MIN_CHUNK: i32 = 64;
const Z_MAX_CHUNK: i32 = std::i32::MAX;
const Z_DEFAULT_CHUNK: i32 = 16 * 1024;
const Z_MIN_MEMLEVEL: i32 = 1;
const Z_MAX_MEMLEVEL: i32 = 9;
const Z_DEFAULT_MEMLEVEL: i32 = 8;
const Z_MIN_LEVEL: i32 = -1;
const Z_MAX_LEVEL: i32 = 9;
const Z_DEFAULT_LEVEL: i32 = Z_DEFAULT_COMPRESSION;
const Z_MIN_WINDOWBITS: i32 = 8;
const Z_MAX_WINDOWBITS: i32 = 15;
const Z_DEFAULT_WINDOWBITS: i32 = 15;

struct ZlibInner {
  // dictionary
  err: u32,
  flush: i32,
  init_done: bool,
  level: i32,
  mem_level: u32,
  mode: u32,
  strategy: u32,
  window_bits: u32,
  write_in_progress: bool,
  pending_close: bool,
  gzib_id_bytes_read: u32,
  strm: z_stream,
}

struct Zlib {
  inner: RefCell<ZlibInner>,
}

impl deno_core::Resource for Zlib {
  fn name(&self) -> Cow<str> {
    "zlib".into()
  }
}

const NONE: u32 = 0;
const DEFLATE: u32 = 1;
const INFLATE: u32 = 2;
const GZIP: u32 = 3;
const GUNZIP: u32 = 4;
const DEFLATERAW: u32 = 5;
const INFLATERAW: u32 = 6;
const UNZIP: u32 = 7;

#[op]
pub fn op_zlib_new(state: &mut OpState, mode: u32) -> Result<u32, AnyError> {
  if mode < DEFLATE || mode > UNZIP {
    return Err(type_error("Bad argument"));
  }
  let mut strm: libz_sys::z_stream = libz_sys::z_stream {
    next_in: std::ptr::null_mut(),
    avail_in: 0,
    total_in: 0,
    next_out: std::ptr::null_mut(),
    avail_out: 0,
    total_out: 0,
    msg: std::ptr::null_mut(),
    state: std::ptr::null_mut(),
    zalloc: flate2_libz_helpers::zalloc,
    zfree: flate2_libz_helpers::zfree,
    opaque: 0 as libz_sys::voidpf,
    data_type: 0,
    adler: 0,
    reserved: 0,
  };
  let inner = ZlibInner {
    err: 0,
    flush: 0,
    init_done: false,
    level: 0,
    mem_level: 0,
    mode,
    strategy: 0,
    window_bits: 0,
    write_in_progress: false,
    pending_close: false,
    gzib_id_bytes_read: 0,
    strm,
  };

  Ok(state.resource_table.add(Zlib { inner }))
}

#[op]
pub fn op_zlib_close(state: &mut OpState, handle: u32) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .ok_or_else(|| type_error("Bad resource id"))?;

  let mut zlib = zlib.inner.borrow_mut();
  if zlib.write_in_progress {
    zlib.pending_close = true;
    return Ok(());
  }

  zlib.pending_close = false;
  check(zlib.init_done, "close before init")?;
  check(zlib.mode <= UNZIP, "invalid mode")?;

  if this.mode == DEFLATE || this.mode == GZIP || this.mode == DEFLATERAW {
    // zlib_deflate.deflateEnd(this.strm.handle);
  } else if (this.mode == INFLATE
    || this.mode == GUNZIP
    || this.mode == INFLATERAW
    || this.mode == UNZIP)
  {
    // zlib_inflate.inflateEnd(this.strm.handle);
  }

  zlib.mode = NONE;

  Ok(())
}

#[op]
pub fn op_zlib_write(
  state: &mut OpState,
  handle: u32,
  flush: i32,
  input: &[u8],
  in_off: u32,
  out: &mut [u8],
  out_off: u32,
  result: &mut [u32],
) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .ok_or_else(|| type_error("Bad resource id"))?;

  let mut zlib = zlib.inner.borrow_mut();
  check(zlib.init_done, "write before init")?;
  check(zlib.mode != NONE, "already finialized")?;
  check(!zlib.write_in_progress, "write already in progress")?;
  check(!zlib.pending_close, "close already in progress")?;

  zlib.write_in_progress = true;

  //   if flush != Z_NO_FLUSH && flush != Z_PARTIAL_FLUSH && flush != Z_SYNC_FLUSH
  //     && flush != Z_FULL_FLUSH && flush != Z_FINISH && flush != Z_BLOCK
  //   {
  //     return Err(type_error("Bad argument"));
  //   }

  // Make sure buffer is large enough to hold the output.
  if out_len < zlib.strm.avail_out {
    return Err(type_error("Bad argument"));
  }

  zlib.strm.avail_in = input.len() as u32;
  zlib.strm.next_in = input.as_ptr() as *mut u8;
  zlib.strm.avail_out = out.len() as u32;
  zlib.strm.next_out = out.as_mut_ptr();

  zlib.flush = flush;

  match self.mode {
    DEFLATE | GZIP | DEFLATERAW => {
      // zlib_deflate.deflate(&mut self.strm, flush);
    }
    _ => unimplemented!(),
  }

  zlib.write_in_progress = false;

  result[0] = zlib.strm.total_out;
  result[1] = zlib.strm.total_in;

  Ok(())
}

#[op]
pub fn op_zlib_init(
  state: &mut OpState,
  handle: u32,
  level: i32,
  window_bits: u32,
  mem_level: u32,
  strategy: u32,
  dictionary: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .ok_or_else(|| type_error("Bad resource id"))?;

  check(window_bits >= 8 && window_bits <= 15, "invalid windowBits")?;
  check(level >= -1 && level <= 9, "invalid level")?;

  // check(
  //   strategy == Z_DEFAULT_STRATEGY
  //     || strategy == Z_FILTERED
  //     || strategy == Z_HUFFMAN_ONLY
  //     || strategy == Z_RLE
  //     || strategy == Z_FIXED,
  //   "invalid strategy",
  // )?;

  let mut zlib = zlib.inner.borrow_mut();
  zlib.level = level;
  zlib.window_bits = window_bits;
  zlib.mem_level = mem_level;
  zlib.strategy = strategy;

  // zlib.flush = Z_NO_FLUSH;
  // zlib.err = Z_OK;

  match zlib.mode {
    GZIP | GUNZIP => zlib.window_bits += 16,
    UNZIP => zlib.window_bits += 32,
    DEFLATERAW | INFLATERAW => zlib.window_bits = -zlib.window_bits,
    _ => {}
  }

  match zlib.mode {
    DEFLATE | GZIP | DEFLATERAW => {
      // zlib_deflate.deflateInit2(
      //   &mut self.strm,
      //   level,
      //   Z_DEFLATED,
      //   window_bits,
      //   mem_level,
      //   strategy,
      // );
    }
    _ => unimplemented!(),
  }

  // zlib.dictionary = dictionary.map(|buf| buf.to_vec());
  zlib.write_in_progress = false;
  zlib.init_done = true;

  Ok(())
}

#[op]
pub fn op_zlib_reset(state: &mut OpState, handle: u32) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .ok_or_else(|| type_error("Bad resource id"))?;

  let mut zlib = zlib.inner.borrow_mut();

  // zlib.err = Z_OK;
  match zlib.mode {
    DEFLATE | GZIP | DEFLATERAW => {
      // zlib_deflate.deflateReset(&mut self.strm);
    }
    _ => unimplemented!(),
  }

  Ok(())
}
