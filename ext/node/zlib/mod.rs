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
mod mode;

use mode::Mode;

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
  mode: Mode,
  strategy: i32,
  window_bits: i32,
  write_in_progress: bool,
  pending_close: bool,
  gzib_id_bytes_read: u32,
  strm: z_stream,
}

const GZIP_HEADER_ID1: u8 = 0x1f;
const GZIP_HEADER_ID2: u8 = 0x8b;

impl ZlibInner {
  fn start_write(
    &mut self,
    input: &[u8],
    in_off: u32,
    in_len: u32,
    out: &mut [u8],
    out_off: u32,
    out_len: u32,
    flush: i32,
  ) -> Result<(), AnyError> {
    check(self.init_done, "write before init")?;
    check(!self.write_in_progress, "write already in progress")?;
    check(!self.pending_close, "close already in progress")?;

    self.write_in_progress = true;

    check(
      flush == Z_NO_FLUSH
        || flush == Z_PARTIAL_FLUSH
        || flush == Z_SYNC_FLUSH
        || flush == Z_FULL_FLUSH
        || flush == Z_FINISH
        || flush == Z_BLOCK,
      "Bad argument",
    )?;

    self.strm.avail_in = in_len;
    // TODO(@littledivy): Crap! Guard against overflow!!
    self.strm.next_in = unsafe { input.as_ptr().offset(in_off as isize) } as _;
    self.strm.avail_out = out_len;
    self.strm.next_out =
      unsafe { out.as_mut_ptr().offset(out_off as isize) } as _;

    self.flush = flush;
    Ok(())
  }

  fn do_write(&mut self, flush: i32) -> Result<(), AnyError> {
    self.flush = flush;
    match self.mode {
      Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => {
        self.err = unsafe { libz_sys::deflate(&mut self.strm, self.flush) };
      }
      Mode::Unzip if self.strm.avail_in > 0 => 'blck: {
        let mut next_expected_header_byte = Some(0);

        if self.gzib_id_bytes_read == 0 {
          let byte = unsafe { *self.strm.next_in.offset(0) };
          if byte == GZIP_HEADER_ID1 {
            self.gzib_id_bytes_read = 1;
            next_expected_header_byte = Some(1);

            // Not enough.
            if self.strm.avail_in == 1 {
              break 'blck;
            }
          } else {
            self.mode = Mode::Inflate;
            next_expected_header_byte = None;
          }
        }

        if self.gzib_id_bytes_read == 1 && next_expected_header_byte.is_some() {
          let byte = unsafe {
            *self.strm.next_in.offset(next_expected_header_byte.unwrap())
          };
          if byte == GZIP_HEADER_ID2 {
            self.gzib_id_bytes_read = 2;
            self.mode = Mode::Gunzip;
          } else {
            self.mode = Mode::Inflate;
          }
        } else if next_expected_header_byte != None {
          return Err(type_error(
            "invalid number of gzip magic number bytes read",
          ));
        }
      }
      _ => {}
    }

    match self.mode {
      Mode::Inflate
        | Mode::Gunzip
        | Mode::InflateRaw
        // We're still reading the header.
        | Mode::Unzip => {
        self.err = unsafe { libz_sys::inflate(&mut self.strm, self.flush) };
        // TODO(@littledivy): Use if let chain when it is stable.
        // https://github.com/rust-lang/rust/issues/53667
        if self.err == Z_NEED_DICT && self.dictionary.is_some() {
          // Data was encoded with dictionary
          let dictionary = self.dictionary.as_ref().unwrap();
          self.err = unsafe {
            libz_sys::inflateSetDictionary(
              &mut self.strm,
              dictionary.as_ptr() as _,
              dictionary.len() as _,
            )
          };

          if self.err == Z_OK {
            self.err = unsafe { libz_sys::inflate(&mut self.strm, self.flush) };
          } else if self.err == Z_DATA_ERROR {
            self.err = Z_NEED_DICT;
          }
        }

        while self.strm.avail_in > 0
          && self.mode == Mode::Gunzip
          && self.err == Z_STREAM_END
          && unsafe { *self.strm.next_in as u8 } != 0x00
        {
          self.err = unsafe { libz_sys::inflateReset(&mut self.strm) };
          self.err = unsafe { libz_sys::inflate(&mut self.strm, self.flush) };
        }
      }
      _ => {}
    }

    if self.err == Z_BUF_ERROR
      && !(self.strm.avail_out != 0 && self.flush == Z_FINISH)
    {
      // Set to Z_OK to avoid reporting the error in JS.
      self.err = Z_OK;
    }

    self.write_in_progress = false;
    Ok(())
  }
}

struct Zlib {
  inner: RefCell<ZlibInner>,
}

impl deno_core::Resource for Zlib {
  fn name(&self) -> Cow<str> {
    "zlib".into()
  }
}

#[op]
pub fn op_zlib_new(state: &mut OpState, mode: i32) -> Result<u32, AnyError> {
  let mode = Mode::try_from(mode)?;
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

  match zlib.mode {
    Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => {
      unsafe { libz_sys::deflateEnd(&mut zlib.strm) };
    }
    Mode::Inflate | Mode::Gunzip | Mode::InflateRaw | Mode::Unzip => {
      unsafe { libz_sys::inflateEnd(&mut zlib.strm) };
    }
    Mode::None => {}
  }

  zlib.mode = Mode::None;

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
  impl Future<Output = Result<(i32, u32, u32), AnyError>> + 'static,
  AnyError,
> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  let mut zlib = resource.inner.borrow_mut();
  zlib.start_write(input, in_off, in_len, out, out_off, out_len, flush)?;

  let state = state.clone();
  Ok(async move {
    let resource = state
      .borrow()
      .resource_table
      .get::<Zlib>(handle)
      .map_err(|_| bad_resource_id())?;
    let mut zlib = resource.inner.borrow_mut();
    zlib.do_write(flush)?;
    Ok((zlib.err, zlib.strm.avail_out, zlib.strm.avail_in))
  })
}

#[op]
pub fn op_zlib_write(
  state: &mut OpState,
  handle: u32,
  flush: i32,
  input: &[u8],
  in_off: u32,
  in_len: u32,
  out: &mut [u8],
  out_off: u32,
  out_len: u32,
  result: &mut [u32],
) -> Result<i32, AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  let mut zlib = resource.inner.borrow_mut();
  zlib.start_write(input, in_off, in_len, out, out_off, out_len, flush)?;
  zlib.do_write(flush)?;

  result[0] = zlib.strm.avail_out as u32;
  result[1] = zlib.strm.avail_in as u32;

  Ok(zlib.err)
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
) -> Result<i32, AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  check(window_bits >= 8 && window_bits <= 15, "invalid windowBits")?;
  check(level >= -1 && level <= 9, "invalid level")?;

  check(mem_level >= 1 && mem_level <= 9, "invalid memLevel")?;

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
    Mode::Gzip | Mode::Gunzip => zlib.window_bits += 16,
    Mode::Unzip => zlib.window_bits += 32,
    Mode::DeflateRaw | Mode::InflateRaw => zlib.window_bits *= -1,
    _ => {}
  }

  zlib.err = match zlib.mode {
    Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => unsafe {
      deflateInit2_(
        &mut zlib.strm,
        level,
        Z_DEFLATED,
        zlib.window_bits,
        zlib.mem_level,
        zlib.strategy,
        zlibVersion(),
        std::mem::size_of::<z_stream>() as i32,
      )
    },
    Mode::Inflate | Mode::Gunzip | Mode::InflateRaw | Mode::Unzip => unsafe {
      inflateInit2_(
        &mut zlib.strm,
        zlib.window_bits,
        zlibVersion(),
        std::mem::size_of::<z_stream>() as i32,
      )
    },
    Mode::None => return Err(type_error("Unknown mode")),
  };

  zlib.dictionary = dictionary.map(|buf| buf.to_vec());
  zlib.write_in_progress = false;
  zlib.init_done = true;

  Ok(zlib.err)
}

#[op]
pub fn op_zlib_reset(
  state: &mut OpState,
  handle: u32,
) -> Result<i32, AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;

  let mut zlib = resource.inner.borrow_mut();

  zlib.err = Z_OK;
  zlib.err = match zlib.mode {
    Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => unsafe {
      libz_sys::deflateReset(&mut zlib.strm)
    },
    Mode::Inflate | Mode::Gunzip | Mode::InflateRaw | Mode::Unzip => unsafe {
      libz_sys::inflateReset(&mut zlib.strm)
    },
    Mode::None => return Err(type_error("Unknown mode")),
  };

  Ok(zlib.err)
}

#[op]
pub fn op_zlib_close_if_pending(
  state: &mut OpState,
  handle: u32,
) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<Zlib>(handle)
    .map_err(|_| bad_resource_id())?;
  let pending_close = {
    let mut zlib = resource.inner.borrow_mut();
    zlib.write_in_progress = false;
    zlib.pending_close
  };
  if pending_close {
    drop(resource);
    state.resource_table.close(handle)?;
  }

  Ok(())
}

#[op]
pub fn op_zlib_get_message(state: &mut OpState, handle: u32) -> Option<String> {
  let resource = state.resource_table.get::<Zlib>(handle);

  match resource {
    Ok(resource) => {
      let zlib = resource.inner.borrow();
      if zlib.err == Z_OK {
        return None;
      }

      let msg = unsafe { std::ffi::CStr::from_ptr(zlib.strm.msg) }
        .to_string_lossy()
        .to_string();
      Some(msg)
    }
    Err(_) => None,
  }
}
