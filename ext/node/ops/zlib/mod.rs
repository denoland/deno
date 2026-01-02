// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use brotli::enc::StandardAlloc;
use brotli::enc::encode::BrotliEncoderDestroyInstance;
use brotli::enc::encode::BrotliEncoderOperation;
use brotli::enc::encode::BrotliEncoderStateStruct;
use brotli::ffi;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8_static_strings;
use deno_error::JsErrorBox;
use libc::c_ulong;
use zlib::*;

mod alloc;
pub mod mode;
mod stream;

use mode::Flush;
use mode::Mode;

use self::alloc::brotli_alloc;
use self::alloc::brotli_free;
use self::stream::StreamWrapper;

#[inline]
fn check(condition: bool, msg: &str) -> Result<(), JsErrorBox> {
  if condition {
    Ok(())
  } else {
    Err(JsErrorBox::type_error(msg.to_string()))
  }
}

#[derive(Default)]
struct ZlibInner {
  dictionary: Option<Vec<u8>>,
  err: i32,
  flush: Flush,
  init_done: bool,
  level: i32,
  mem_level: i32,
  mode: Mode,
  strategy: i32,
  window_bits: i32,
  write_in_progress: bool,
  pending_close: bool,
  gzib_id_bytes_read: u32,
  result_buffer: Option<*mut u32>,
  callback: Option<v8::Global<v8::Function>>,
  strm: StreamWrapper,
}

const GZIP_HEADER_ID1: u8 = 0x1f;
const GZIP_HEADER_ID2: u8 = 0x8b;

impl ZlibInner {
  #[allow(clippy::too_many_arguments)]
  fn start_write(
    &mut self,
    input: &[u8],
    in_off: u32,
    in_len: u32,
    out: &mut [u8],
    out_off: u32,
    out_len: u32,
    flush: Flush,
  ) -> Result<(), JsErrorBox> {
    check(self.init_done, "write before init")?;
    check(!self.write_in_progress, "write already in progress")?;
    check(!self.pending_close, "close already in progress")?;

    self.write_in_progress = true;

    let next_in = input
      .get(in_off as usize..in_off as usize + in_len as usize)
      .ok_or_else(|| JsErrorBox::type_error("invalid input range"))?
      .as_ptr() as *mut _;
    let next_out = out
      .get_mut(out_off as usize..out_off as usize + out_len as usize)
      .ok_or_else(|| JsErrorBox::type_error("invalid output range"))?
      .as_mut_ptr();

    self.strm.avail_in = in_len;
    self.strm.next_in = next_in;
    self.strm.avail_out = out_len;
    self.strm.next_out = next_out;

    self.flush = flush;
    Ok(())
  }

  fn do_write(&mut self, flush: Flush) -> Result<(), JsErrorBox> {
    self.flush = flush;
    match self.mode {
      Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => {
        self.err = self.strm.deflate(flush);
      }
      // Auto-detect mode.
      Mode::Unzip if self.strm.avail_in > 0 => 'blck: {
        let mut next_expected_header_byte = Some(0);
        // SAFETY: `self.strm.next_in` is valid pointer to the input buffer.
        // `self.strm.avail_in` is the length of the input buffer that is only set by
        // `start_write`.
        let strm = unsafe {
          std::slice::from_raw_parts(
            self.strm.next_in,
            self.strm.avail_in as usize,
          )
        };

        if self.gzib_id_bytes_read == 0 {
          if strm[0] == GZIP_HEADER_ID1 {
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

        if self.gzib_id_bytes_read == 1 {
          let byte = match next_expected_header_byte {
            Some(i) => strm[i],
            None => break 'blck,
          };
          if byte == GZIP_HEADER_ID2 {
            self.gzib_id_bytes_read = 2;
            self.mode = Mode::Gunzip;
          } else {
            self.mode = Mode::Inflate;
          }
        } else if next_expected_header_byte.is_some() {
          return Err(JsErrorBox::type_error(
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
        self.err = self.strm.inflate(self.flush);
        // TODO(@littledivy): Use if let chain when it is stable.
        // https://github.com/rust-lang/rust/issues/53667
        //
        // Data was encoded with dictionary
        if let (Z_NEED_DICT, Some(dictionary)) = (self.err, &self.dictionary) {
          self.err = self.strm.inflate_set_dictionary(dictionary);

          if self.err == Z_OK {
            self.err = self.strm.inflate(flush);
          } else if self.err == Z_DATA_ERROR {
            self.err = Z_NEED_DICT;
          }
        }

        while self.strm.avail_in > 0
          && self.mode == Mode::Gunzip
          && self.err == Z_STREAM_END
          // SAFETY: `strm` is a valid pointer to zlib strm.
          // `strm.next_in` is initialized to the input buffer.
          && unsafe { *self.strm.next_in } != 0x00
        {
          self.err = self.strm.reset(self.mode);
          self.err = self.strm.inflate(flush);
        }
      }
      _ => {}
    }

    let done = self.strm.avail_out != 0 && self.flush == Flush::Finish;
    // We're are not done yet, but output buffer is full
    if self.err == Z_BUF_ERROR && !done {
      // Set to Z_OK to avoid reporting the error in JS.
      self.err = Z_OK;
    }

    self.write_in_progress = false;
    Ok(())
  }

  fn init_stream(&mut self) -> Result<(), JsErrorBox> {
    match self.mode {
      Mode::Gzip | Mode::Gunzip => self.window_bits += 16,
      Mode::Unzip => self.window_bits += 32,
      Mode::DeflateRaw | Mode::InflateRaw => self.window_bits *= -1,
      _ => {}
    }

    self.err = match self.mode {
      Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => self.strm.deflate_init(
        self.level,
        self.window_bits,
        self.mem_level,
        self.strategy,
      ),
      Mode::Inflate | Mode::Gunzip | Mode::InflateRaw | Mode::Unzip => {
        self.strm.inflate_init(self.window_bits)
      }
      Mode::None => return Err(JsErrorBox::type_error("Unknown mode")),
    };

    self.write_in_progress = false;
    self.init_done = true;

    Ok(())
  }

  fn close(&mut self) -> Result<bool, JsErrorBox> {
    if self.write_in_progress {
      self.pending_close = true;
      return Ok(false);
    }

    self.pending_close = false;
    check(self.init_done, "close before init")?;

    self.strm.end(self.mode);
    self.mode = Mode::None;
    Ok(true)
  }

  fn reset_stream(&mut self) {
    self.err = self.strm.reset(self.mode);
  }

  fn get_error_info(&self) -> Option<(i32, String)> {
    let err_str = match self.err {
      Z_OK | Z_BUF_ERROR => {
        if self.strm.avail_out != 0 && self.flush == Flush::Finish {
          "unexpected end of file"
        } else {
          return None;
        }
      }
      Z_STREAM_END => return None,
      Z_NEED_DICT => {
        if self.dictionary.is_none() {
          "Missing dictionary"
        } else {
          "Bad dictionary"
        }
      }
      _ => "Zlib error",
    };

    let msg = self.strm.msg;
    Some((
      self.err,
      if !msg.is_null() {
        // SAFETY: `msg` is a valid pointer to a null-terminated string.
        unsafe { std::ffi::CStr::from_ptr(msg).to_str().unwrap().to_string() }
      } else {
        err_str.to_string()
      },
    ))
  }

  fn check_error(
    error_info: Option<(i32, String)>,
    scope: &mut v8::PinScope<'_, '_>,
    this: &v8::Global<v8::Object>,
  ) -> bool {
    let Some((err, msg)) = error_info else {
      return true; // No error, nothing to report.
    };

    let this = v8::Local::new(scope, this);
    v8_static_strings! {
      ONERROR_STR = "onerror",
    }

    let onerror_str = ONERROR_STR.v8_string(scope).unwrap();
    let onerror = this.get(scope, onerror_str.into()).unwrap();
    let cb = v8::Local::<v8::Function>::try_from(onerror).unwrap();

    let msg = v8::String::new(scope, &msg).unwrap();
    let err = v8::Integer::new(scope, err);

    cb.call(scope, this.into(), &[msg.into(), err.into()]);

    false
  }
}

pub struct Zlib {
  inner: RefCell<Option<ZlibInner>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for Zlib {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Zlib"
  }
}

impl deno_core::Resource for Zlib {
  fn name(&self) -> Cow<'_, str> {
    "zlib".into()
  }
}

#[op2]
impl Zlib {
  #[constructor]
  #[cppgc]
  fn new(#[smi] mode: Option<i32>) -> Result<Zlib, mode::ModeError> {
    let mode = mode.unwrap_or(Mode::Deflate as i32);
    let mode = Mode::try_from(mode)?;

    let inner = ZlibInner {
      mode,
      ..Default::default()
    };

    Ok(Zlib {
      inner: RefCell::new(Some(inner)),
    })
  }

  #[fast]
  pub fn close(&self) -> Result<(), ZlibError> {
    let mut resource = self.inner.borrow_mut();
    let zlib = resource.as_mut().ok_or(ZlibError::NotInitialized)?;

    // If there is a pending write, defer the close until the write is done.
    zlib.close()?;

    Ok(())
  }

  #[fast]
  #[smi]
  pub fn reset(&self) -> Result<i32, ZlibError> {
    let mut zlib = self.inner.borrow_mut();
    let zlib = zlib.as_mut().ok_or(ZlibError::NotInitialized)?;

    zlib.reset_stream();

    Ok(zlib.err)
  }

  #[smi]
  pub fn init(
    &self,
    #[smi] window_bits: i32,
    #[smi] level: i32,
    #[smi] mem_level: i32,
    #[smi] strategy: i32,
    #[buffer] write_result: &mut [u32],
    #[global] callback: v8::Global<v8::Function>,
    #[buffer] dictionary: Option<&[u8]>,
  ) -> Result<i32, ZlibError> {
    let mut zlib = self.inner.borrow_mut();
    let zlib = zlib.as_mut().ok_or(ZlibError::NotInitialized)?;

    if !((window_bits == 0)
      && matches!(zlib.mode, Mode::Inflate | Mode::Gunzip | Mode::Unzip))
    {
      check((8..=15).contains(&window_bits), "invalid windowBits")?;
    }

    check((-1..=9).contains(&level), "invalid level")?;

    check((1..=9).contains(&mem_level), "invalid memLevel")?;

    check(
      strategy == Z_DEFAULT_STRATEGY
        || strategy == Z_FILTERED
        || strategy == Z_HUFFMAN_ONLY
        || strategy == Z_RLE
        || strategy == Z_FIXED,
      "invalid strategy",
    )?;

    zlib.level = level;
    zlib.window_bits = window_bits;
    zlib.mem_level = mem_level;
    zlib.strategy = strategy;

    zlib.flush = Flush::None;
    zlib.err = Z_OK;

    zlib.init_stream()?;

    zlib.dictionary = dictionary.map(|buf| buf.to_vec());

    zlib.result_buffer = Some(write_result.as_mut_ptr());
    zlib.callback = Some(callback);

    Ok(zlib.err)
  }

  #[fast]
  #[reentrant]
  pub fn write_sync(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[smi] flush: i32,
    #[buffer] input: &[u8],
    #[smi] in_off: u32,
    #[smi] in_len: u32,
    #[buffer] out: &mut [u8],
    #[smi] out_off: u32,
    #[smi] out_len: u32,
  ) -> Result<(), ZlibError> {
    let err_info = {
      let mut zlib = self.inner.borrow_mut();
      let zlib = zlib.as_mut().ok_or(ZlibError::NotInitialized)?;

      let flush = Flush::try_from(flush)?;
      zlib.start_write(input, in_off, in_len, out, out_off, out_len, flush)?;
      zlib.do_write(flush)?;

      // SAFETY: `zlib.result_buffer` is a valid pointer to a mutable slice of u32 of length 2.
      let result = unsafe {
        std::slice::from_raw_parts_mut(zlib.result_buffer.unwrap(), 2)
      };
      result[0] = zlib.strm.avail_out;
      result[1] = zlib.strm.avail_in;
      zlib.get_error_info()
    };

    ZlibInner::check_error(err_info, scope, &this);
    Ok(())
  }

  #[fast]
  #[reentrant]
  fn write(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[smi] flush: i32,
    #[buffer] input: &[u8],
    #[smi] in_off: u32,
    #[smi] in_len: u32,
    #[buffer] out: &mut [u8],
    #[smi] out_off: u32,
    #[smi] out_len: u32,
  ) -> Result<(), ZlibError> {
    let (err_info, callback) = {
      let mut zlib = self.inner.borrow_mut();
      let zlib = zlib.as_mut().ok_or(ZlibError::NotInitialized)?;

      let flush = Flush::try_from(flush)?;
      zlib.start_write(input, in_off, in_len, out, out_off, out_len, flush)?;
      zlib.do_write(flush)?;

      // SAFETY: `zlib.result_buffer` is a valid pointer to a mutable slice of u32 of length 2.
      let result = unsafe {
        std::slice::from_raw_parts_mut(zlib.result_buffer.unwrap(), 2)
      };
      result[0] = zlib.strm.avail_out;
      result[1] = zlib.strm.avail_in;
      (
        zlib.get_error_info(),
        v8::Local::new(
          scope,
          zlib.callback.as_ref().expect("callback not set"),
        ),
      )
    };

    if !ZlibInner::check_error(err_info, scope, &this) {
      return Ok(());
    }

    let this = v8::Local::new(scope, &this);
    let _ = callback.call(scope, this.into(), &[]);

    Ok(())
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ZlibError {
  #[class(type)]
  #[error("zlib not initialized")]
  NotInitialized,
  #[class(inherit)]
  #[error(transparent)]
  Mode(
    #[from]
    #[inherit]
    mode::ModeError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Other(
    #[from]
    #[inherit]
    JsErrorBox,
  ),
}

#[op2]
#[string]
pub fn op_zlib_err_msg(
  #[cppgc] resource: &Zlib,
) -> Result<Option<String>, ZlibError> {
  let mut zlib = resource.inner.borrow_mut();
  let zlib = zlib.as_mut().ok_or(ZlibError::NotInitialized)?;

  let msg = zlib.strm.msg;
  if msg.is_null() {
    return Ok(None);
  }

  // SAFETY: `msg` is a valid pointer to a null-terminated string.
  let msg = unsafe {
    std::ffi::CStr::from_ptr(msg)
      .to_str()
      .map_err(|_| JsErrorBox::type_error("invalid error message"))?
      .to_string()
  };

  Ok(Some(msg))
}

#[op2(fast)]
pub fn op_zlib_close_if_pending(
  #[cppgc] resource: &Zlib,
) -> Result<(), ZlibError> {
  let pending_close = {
    let mut zlib = resource.inner.borrow_mut();
    let zlib = zlib.as_mut().ok_or(ZlibError::NotInitialized)?;

    zlib.write_in_progress = false;
    zlib.pending_close
  };
  if pending_close && let Some(mut res) = resource.inner.borrow_mut().take() {
    let _ = res.close();
  }

  Ok(())
}

struct BrotliEncoderCtx {
  inst: BrotliEncoderStateStruct<StandardAlloc>,
  write_result: *mut u32,
  callback: v8::Global<v8::Function>,
}

pub struct BrotliEncoder {
  ctx: Rc<RefCell<Option<BrotliEncoderCtx>>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for BrotliEncoder {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"BrotliEncoder"
  }
}

fn encoder_param(i: u32) -> brotli::enc::encode::BrotliEncoderParameter {
  const _: () = {
    assert!(
      std::mem::size_of::<brotli::enc::encode::BrotliEncoderParameter>()
        == std::mem::size_of::<u32>(),
    );
  };
  // SAFETY: `i` is a valid u32 value that corresponds to a BrotliEncoderParameter.
  unsafe { std::mem::transmute(i) }
}

#[op2]
impl BrotliEncoder {
  #[constructor]
  #[cppgc]
  fn new(#[smi] _mode: i32) -> BrotliEncoder {
    BrotliEncoder {
      ctx: Rc::new(RefCell::new(None)),
    }
  }

  fn init(
    &self,
    #[buffer] params: &[u32],
    #[buffer] write_result: &mut [u32],
    #[global] callback: v8::Global<v8::Function>,
  ) {
    let inst = {
      let mut state = BrotliEncoderStateStruct::new(StandardAlloc::default());

      for (i, &value) in params.iter().enumerate() {
        if value == 0xFFFFFFFF {
          continue; // Skip setting the parameter, same as C API.
        }
        state.set_parameter(encoder_param(i as u32), value);
      }

      state
    };

    self.ctx.borrow_mut().replace(BrotliEncoderCtx {
      inst,
      write_result: write_result.as_mut_ptr(),
      callback,
    });
  }

  #[fast]
  fn params(&self) {
    // no-op
  }

  #[fast]
  fn reset(&self) {}

  #[fast]
  #[reentrant]
  pub fn write(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[smi] flush: u8,
    #[buffer] input: &[u8],
    #[smi] in_off: u32,
    #[smi] in_len: u32,
    #[buffer] out: &mut [u8],
    #[smi] out_off: u32,
    #[smi] out_len: u32,
  ) -> Result<(), JsErrorBox> {
    let mut avail_in = in_len as usize;
    let mut avail_out = out_len as usize;
    // SAFETY: `inst`, `next_in`, `next_out`, `avail_in`, and `avail_out` are valid pointers.
    let callback = unsafe {
      let mut ctx = self.ctx.borrow_mut();
      let ctx = ctx.as_mut().expect("BrotliDecoder not initialized");

      ctx.inst.compress_stream(
        std::mem::transmute::<u8, BrotliEncoderOperation>(flush),
        &mut avail_in,
        input,
        &mut (in_off as usize),
        &mut avail_out,
        out,
        &mut (out_off as usize),
        &mut None,
        &mut |_, _, _, _| (),
      );

      // SAFETY: `write_result` is a valid pointer to a mutable slice of u32 of length 2.
      let result = std::slice::from_raw_parts_mut(ctx.write_result, 2);
      result[0] = avail_out as u32;
      result[1] = avail_in as u32;

      v8::Local::new(scope, &ctx.callback)
    };
    let this = v8::Local::new(scope, &this);
    let _ = callback.call(scope, this.into(), &[]);

    Ok(())
  }

  #[fast]
  pub fn write_sync(
    &self,
    #[smi] flush: u8,
    #[buffer] input: &[u8],
    #[smi] in_off: u32,
    #[smi] in_len: u32,
    #[buffer] out: &mut [u8],
    #[smi] out_off: u32,
    #[smi] out_len: u32,
  ) -> Result<(), JsErrorBox> {
    let mut ctx = self.ctx.borrow_mut();
    let ctx = ctx.as_mut().expect("BrotliEncoder not initialized");

    let mut avail_in = in_len as usize;
    let mut avail_out = out_len as usize;
    // SAFETY: `inst`, `next_in`, `next_out`, `avail_in`, and `avail_out` are valid pointers.
    unsafe {
      ctx.inst.compress_stream(
        std::mem::transmute::<u8, BrotliEncoderOperation>(flush),
        &mut avail_in,
        input,
        &mut (in_off as usize),
        &mut avail_out,
        out,
        &mut (out_off as usize),
        &mut None,
        &mut |_, _, _, _| (),
      );

      // SAFETY: `ctx.write_result` is a valid pointer to a mutable slice of u32 of length 2.
      let result = std::slice::from_raw_parts_mut(ctx.write_result, 2);
      result[0] = avail_out as u32;
      result[1] = avail_in as u32;
    };

    Ok(())
  }

  #[fast]
  fn close(&self) {
    let mut ctx = self.ctx.borrow_mut();
    if let Some(mut ctx) = ctx.take() {
      BrotliEncoderDestroyInstance(&mut ctx.inst);
    }
  }
}

struct BrotliDecoderCtx {
  inst: *mut ffi::decompressor::ffi::BrotliDecoderState,
  write_result: *mut u32,
  callback: v8::Global<v8::Function>,
}

pub struct BrotliDecoder {
  ctx: Rc<RefCell<Option<BrotliDecoderCtx>>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for BrotliDecoder {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"BrotliDecoder"
  }
}

fn decoder_param(
  i: u32,
) -> Option<ffi::decompressor::ffi::interface::BrotliDecoderParameter> {
  const _: () = {
    assert!(
      std::mem::size_of::<
        ffi::decompressor::ffi::interface::BrotliDecoderParameter,
      >()
        == std::mem::size_of::<u32>(),
    );
  };
  match i {
    0 => Some(ffi::decompressor::ffi::interface::BrotliDecoderParameter::BROTLI_DECODER_PARAM_DISABLE_RING_BUFFER_REALLOCATION),
    1 => Some(ffi::decompressor::ffi::interface::BrotliDecoderParameter::BROTLI_DECODER_PARAM_LARGE_WINDOW),
    _ => None
  }
}

#[op2]
impl BrotliDecoder {
  #[constructor]
  #[cppgc]
  fn new(#[smi] _mode: i32) -> BrotliDecoder {
    BrotliDecoder {
      ctx: Rc::new(RefCell::new(None)),
    }
  }

  fn init(
    &self,
    #[buffer] params: &[u32],
    #[buffer] write_result: &mut [u32],
    #[global] callback: v8::Global<v8::Function>,
  ) {
    // SAFETY: creates new brotli decoder instance. `params` is a valid slice of u32 values.
    let inst = unsafe {
      let state = ffi::decompressor::ffi::BrotliDecoderCreateInstance(
        Some(brotli_alloc),
        Some(brotli_free),
        std::ptr::null_mut(),
      );
      for (i, &value) in params.iter().enumerate() {
        if let Some(param) = decoder_param(i as u32) {
          ffi::decompressor::ffi::BrotliDecoderSetParameter(
            state, param, value,
          );
        }
      }

      state
    };

    self.ctx.borrow_mut().replace(BrotliDecoderCtx {
      inst,
      write_result: write_result.as_mut_ptr(),
      callback,
    });
  }

  #[fast]
  fn params(&self) {
    // no-op
  }

  #[fast]
  fn reset(&self) {}

  #[fast]
  #[reentrant]
  pub fn write(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[smi] _flush: i32,
    #[buffer] input: &[u8],
    #[smi] in_off: u32,
    #[smi] in_len: u32,
    #[buffer] out: &mut [u8],
    #[smi] out_off: u32,
    #[smi] out_len: u32,
  ) -> Result<(), JsErrorBox> {
    let callback = {
      let ctx = self.ctx.borrow();
      let ctx = ctx.as_ref().expect("BrotliDecoder not initialized");

      let mut next_in = input
        .get(in_off as usize..in_off as usize + in_len as usize)
        .ok_or_else(|| JsErrorBox::type_error("invalid input range"))?
        .as_ptr();
      let mut next_out = out
        .get_mut(out_off as usize..out_off as usize + out_len as usize)
        .ok_or_else(|| JsErrorBox::type_error("invalid output range"))?
        .as_mut_ptr();

      let mut avail_in = in_len as usize;
      let mut avail_out = out_len as usize;

      // SAFETY: `inst`, `next_in`, `next_out`, `avail_in`, and `avail_out` are valid pointers.
      unsafe {
        ffi::decompressor::ffi::BrotliDecoderDecompressStream(
          ctx.inst,
          &mut avail_in,
          &mut next_in,
          &mut avail_out,
          &mut next_out,
          std::ptr::null_mut(),
        );

        // SAFETY: `write_result` is a valid pointer to a mutable slice of u32 of length 2.
        let result = std::slice::from_raw_parts_mut(ctx.write_result, 2);
        result[0] = avail_out as u32;
        result[1] = avail_in as u32;
      }

      v8::Local::new(scope, &ctx.callback)
    };

    let this = v8::Local::new(scope, &this);
    let _ = callback.call(scope, this.into(), &[]);

    Ok(())
  }

  #[fast]
  pub fn write_sync(
    &self,
    #[smi] _flush: i32,
    #[buffer] input: &[u8],
    #[smi] in_off: u32,
    #[smi] in_len: u32,
    #[buffer] out: &mut [u8],
    #[smi] out_off: u32,
    #[smi] out_len: u32,
  ) -> Result<(), JsErrorBox> {
    let mut ctx = self.ctx.borrow_mut();
    let ctx = ctx.as_mut().expect("BrotliDecoder not initialized");

    let mut next_in = input
      .get(in_off as usize..in_off as usize + in_len as usize)
      .ok_or_else(|| JsErrorBox::type_error("invalid input range"))?
      .as_ptr();
    let mut next_out = out
      .get_mut(out_off as usize..out_off as usize + out_len as usize)
      .ok_or_else(|| JsErrorBox::type_error("invalid output range"))?
      .as_mut_ptr();

    let mut avail_in = in_len as usize;
    let mut avail_out = out_len as usize;

    // SAFETY: `ctx.inst` is a valid pointer to a BrotliDecoderState.
    unsafe {
      ffi::decompressor::ffi::BrotliDecoderDecompressStream(
        ctx.inst,
        &mut avail_in,
        &mut next_in,
        &mut avail_out,
        &mut next_out,
        std::ptr::null_mut(),
      );

      // SAFETY: `ctx.write_result` is a valid pointer to a mutable slice of u32 of length 2.
      let result = std::slice::from_raw_parts_mut(ctx.write_result, 2);
      result[0] = avail_out as u32;
      result[1] = avail_in as u32;
    }

    Ok(())
  }

  #[fast]
  fn close(&self) {
    let mut ctx = self.ctx.borrow_mut();
    if let Some(ctx) = ctx.take() {
      // SAFETY: `ctx.inst` is a valid pointer to a BrotliDecoderState.
      unsafe {
        ffi::decompressor::ffi::BrotliDecoderDestroyInstance(ctx.inst);
      }
    }
  }
}

#[op2(fast)]
pub fn op_zlib_crc32_string(#[string] data: &str, value: u32) -> u32 {
  // SAFETY: `data` is a valid buffer.
  unsafe {
    zlib::crc32(value as c_ulong, data.as_ptr(), data.len() as u32) as u32
  }
}

#[op2(fast)]
pub fn op_zlib_crc32(#[buffer] data: &[u8], value: u32) -> u32 {
  // SAFETY: `data` is a valid buffer.
  unsafe {
    zlib::crc32(value as c_ulong, data.as_ptr(), data.len() as u32) as u32
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn zlib_start_write() {
    // buffer, length, should pass
    type WriteVector = (&'static [u8], u32, u32, bool);
    const WRITE_VECTORS: [WriteVector; 8] = [
      (b"Hello", 5, 0, true),
      (b"H", 1, 0, true),
      (b"", 0, 0, true),
      // Overrun the buffer
      (b"H", 5, 0, false),
      (b"ello", 5, 0, false),
      (b"Hello", 5, 1, false),
      (b"H", 1, 1, false),
      (b"", 0, 1, false),
    ];

    for (input, len, offset, expected) in WRITE_VECTORS.iter() {
      let mut stream = ZlibInner {
        mode: Mode::Inflate,
        ..Default::default()
      };

      stream.init_stream().unwrap();
      assert_eq!(stream.err, Z_OK);
      assert_eq!(
        stream
          .start_write(input, *offset, *len, &mut [], 0, 0, Flush::None)
          .is_ok(),
        *expected
      );
      assert_eq!(stream.err, Z_OK);
      stream.close().unwrap();
    }
  }
}
