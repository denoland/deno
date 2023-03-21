// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use libz_sys::*;
use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

mod alloc;

#[inline]
fn check(condition: bool, msg: &str) -> Result<(), AnyError> {
  if condition {
    Ok(())
  } else {
    Err(type_error(msg.to_string()))
  }
}

struct ZlibInner {
  dictionary: Option<Vec<u8>>,
  err: i32,
  flush: i32,
  init_done: bool,
  level: i32,
  mem_level: i32,
  mode: i32,
  strategy: i32,
  window_bits: i32,
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

const NONE: i32 = 0;
const DEFLATE: i32 = 1;
const INFLATE: i32 = 2;
const GZIP: i32 = 3;
const GUNZIP: i32 = 4;
const DEFLATERAW: i32 = 5;
const INFLATERAW: i32 = 6;
const UNZIP: i32 = 7;

#[op]
pub fn op_zlib_new(state: &mut OpState, mode: i32) -> Result<u32, AnyError> {
  if mode < DEFLATE || mode > UNZIP {
    return Err(type_error("Bad argument"));
  }
  let strm: libz_sys::z_stream = libz_sys::z_stream {
    next_in: std::ptr::null_mut(),
    avail_in: 0,
    total_in: 0,
    next_out: std::ptr::null_mut(),
    avail_out: 0,
    total_out: 0,
    msg: std::ptr::null_mut(),
    state: std::ptr::null_mut(),
    zalloc: alloc::zalloc,
    zfree: alloc::zfree,
    opaque: 0 as libz_sys::voidpf,
    data_type: 0,
    adler: 0,
    reserved: 0,
  };
  let inner = ZlibInner {
    strm,
    mode,
    dictionary: None,
    err: 0,
    flush: 0,
    init_done: false,
    level: 0,
    mem_level: 0,
    strategy: 0,
    window_bits: 0,
    write_in_progress: false,
    pending_close: false,
    gzib_id_bytes_read: 0,
  };

  Ok(state.resource_table.add(Zlib {
    inner: RefCell::new(inner),
  }))
}

#[op]
pub fn op_zlib_close(state: &mut OpState, handle: u32) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  let mut zlib = resource.inner.borrow_mut();
  if zlib.write_in_progress {
    zlib.pending_close = true;
    return Ok(());
  }

  zlib.pending_close = false;
  check(zlib.init_done, "close before init")?;
  check(zlib.mode <= UNZIP, "invalid mode")?;

  if zlib.mode == DEFLATE || zlib.mode == GZIP || zlib.mode == DEFLATERAW {
    unsafe { libz_sys::deflateEnd(&mut zlib.strm) };
  } else if zlib.mode == INFLATE
    || zlib.mode == GUNZIP
    || zlib.mode == INFLATERAW
    || zlib.mode == UNZIP
  {
    unsafe { libz_sys::inflateEnd(&mut zlib.strm) };
  }

  zlib.mode = NONE;

  Ok(())
}

#[op]
pub fn op_zlib_write_async(
  state: Rc<RefCell<OpState>>,
  handle: u32,
  flush: i32,
  input: &[u8],
  in_off: u32,
  in_len: u32,
  out: &mut [u8],
  out_off: u32,
  out_len: u32,
) -> Result<
  impl Future<Output = Result<(u32, u32), AnyError>> + 'static,
  AnyError,
> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  let mut zlib = resource.inner.borrow_mut();
  check(zlib.init_done, "write before init")?;
  check(zlib.mode != NONE, "already finialized")?;
  check(!zlib.write_in_progress, "write already in progress")?;
  check(!zlib.pending_close, "close already in progress")?;

  zlib.write_in_progress = true;

  if flush != Z_NO_FLUSH
    && flush != Z_PARTIAL_FLUSH
    && flush != Z_SYNC_FLUSH
    && flush != Z_FULL_FLUSH
    && flush != Z_FINISH
    && flush != Z_BLOCK
  {
    return Err(type_error("Bad argument"));
  }

  zlib.strm.avail_in = in_len;
  // TODO(@littledivy): Crap! Guard against overflow!!
  zlib.strm.next_in = unsafe { input.as_ptr().offset(in_off as isize) } as _;
  zlib.strm.avail_out = out_len;
  zlib.strm.next_out =
    unsafe { out.as_mut_ptr().offset(out_off as isize) } as _;

  zlib.flush = flush;

  let state = state.clone();
  Ok(async move {
    let resource = state
      .borrow()
      .resource_table
      .get::<Zlib>(handle)
      .map_err(|_| bad_resource_id())?;
    let mut zlib = resource.inner.borrow_mut();
    match zlib.mode {
      DEFLATE | GZIP | DEFLATERAW => {
        unsafe { libz_sys::deflate(&mut zlib.strm, flush) };
      }
      _ => {
        unsafe { libz_sys::inflate(&mut zlib.strm, flush) };
      }
    }

    zlib.write_in_progress = false;

    Ok((zlib.strm.avail_out, zlib.strm.avail_in))
  })
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
    .map_err(|_| bad_resource_id())?;

  let mut zlib = resource.inner.borrow_mut();
  check(zlib.init_done, "write before init")?;
  check(zlib.mode != NONE, "already finialized")?;
  check(!zlib.write_in_progress, "write already in progress")?;
  check(!zlib.pending_close, "close already in progress")?;

  zlib.write_in_progress = true;

  if flush != Z_NO_FLUSH
    && flush != Z_PARTIAL_FLUSH
    && flush != Z_SYNC_FLUSH
    && flush != Z_FULL_FLUSH
    && flush != Z_FINISH
    && flush != Z_BLOCK
  {
    return Err(type_error("Bad argument"));
  }

  zlib.strm.avail_in = input.len() as u32;
  zlib.strm.next_in = input.as_ptr() as *mut u8;
  zlib.strm.avail_out = out.len() as u32;
  zlib.strm.next_out = out.as_mut_ptr();

  zlib.flush = flush;

  match zlib.mode {
    DEFLATE | GZIP | DEFLATERAW => {
      unsafe { libz_sys::deflate(&mut zlib.strm, flush) };
    }
    _ => {
      unsafe { libz_sys::inflate(&mut zlib.strm, flush) };
    }
  }

  zlib.write_in_progress = false;

  result[0] = zlib.strm.total_out as u32;
  result[1] = zlib.strm.total_in as u32;

  Ok(())
}

#[op]
pub fn op_zlib_init(
  state: &mut OpState,
  handle: u32,
  level: i32,
  window_bits: i32,
  mem_level: i32,
  strategy: i32,
  dictionary: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  check(window_bits >= 8 && window_bits <= 15, "invalid windowBits")?;
  check(level >= -1 && level <= 9, "invalid level")?;

  check(
    strategy == Z_DEFAULT_STRATEGY
      || strategy == Z_FILTERED
      || strategy == Z_HUFFMAN_ONLY
      || strategy == Z_RLE
      || strategy == Z_FIXED,
    "invalid strategy",
  )?;

  let mut zlib = resource.inner.borrow_mut();
  zlib.level = level;
  zlib.window_bits = window_bits;
  zlib.mem_level = mem_level;
  zlib.strategy = strategy;

  zlib.flush = Z_NO_FLUSH;
  zlib.err = Z_OK;

  match zlib.mode {
    GZIP | GUNZIP => zlib.window_bits += 16,
    UNZIP => zlib.window_bits += 32,
    DEFLATERAW | INFLATERAW => zlib.window_bits = -zlib.window_bits,
    _ => {}
  }

  match zlib.mode {
    DEFLATE | GZIP | DEFLATERAW => unsafe {
      deflateInit2_(
        &mut zlib.strm,
        level,
        Z_DEFLATED,
        window_bits,
        mem_level,
        strategy,
        zlibVersion(),
        std::mem::size_of::<z_stream>() as i32,
      );
    },
    _ => unsafe {
      inflateInit2_(
        &mut zlib.strm,
        window_bits,
        zlibVersion(),
        std::mem::size_of::<z_stream>() as i32,
      );
    },
  }

  zlib.dictionary = dictionary.map(|buf| buf.to_vec());
  zlib.write_in_progress = false;
  zlib.init_done = true;

  Ok(())
}

#[op]
pub fn op_zlib_reset(state: &mut OpState, handle: u32) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  let mut zlib = resource.inner.borrow_mut();

  zlib.err = Z_OK;
  match zlib.mode {
    DEFLATE | GZIP | DEFLATERAW => {
      unsafe { libz_sys::deflateReset(&mut zlib.strm) };
    }
    _ => {
      unsafe { libz_sys::inflateReset(&mut zlib.strm) };
    }
  }

  Ok(())
}
