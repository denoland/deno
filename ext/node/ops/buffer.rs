// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;

#[op2(fast)]
pub fn op_is_ascii(#[buffer] buf: &[u8]) -> bool {
  buf.is_ascii()
}

#[op2(fast)]
pub fn op_is_utf8(#[buffer] buf: &[u8]) -> bool {
  std::str::from_utf8(buf).is_ok()
}

#[op2]
#[buffer]
pub fn op_transcode(
  #[buffer] source: &[u8],
  #[string] from_encoding: &str,
  #[string] to_encoding: &str,
) -> Result<Vec<u8>, JsErrorBox> {
  match (from_encoding, to_encoding) {
    ("utf8", "ascii") => Ok(utf8_to_ascii(source)),
    ("utf8", "latin1") => Ok(utf8_to_latin1(source)),
    ("utf8", "utf16le") => utf8_to_utf16le(source),
    ("utf16le", "utf8") => utf16le_to_utf8(source),
    ("latin1", "utf16le") | ("ascii", "utf16le") => {
      Ok(latin1_ascii_to_utf16le(source))
    }
    (from, to) => Err(JsErrorBox::generic(format!(
      "Unable to transcode Buffer {from}->{to}"
    ))),
  }
}

fn latin1_ascii_to_utf16le(source: &[u8]) -> Vec<u8> {
  let mut result = Vec::with_capacity(source.len() * 2);
  for &byte in source {
    result.push(byte);
    result.push(0);
  }
  result
}

fn utf16le_to_utf8(source: &[u8]) -> Result<Vec<u8>, JsErrorBox> {
  let ucs2_vec: Vec<u16> = source
    .chunks(2)
    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
    .collect();
  String::from_utf16(&ucs2_vec)
    .map(|utf8_string| utf8_string.into_bytes())
    .map_err(|e| JsErrorBox::generic(format!("Invalid UTF-16 sequence: {}", e)))
}

fn utf8_to_utf16le(source: &[u8]) -> Result<Vec<u8>, JsErrorBox> {
  let utf8_string =
    std::str::from_utf8(source).map_err(JsErrorBox::from_err)?;
  let ucs2_vec: Vec<u16> = utf8_string.encode_utf16().collect();
  let bytes: Vec<u8> = ucs2_vec.iter().flat_map(|&x| x.to_le_bytes()).collect();
  Ok(bytes)
}

fn utf8_to_latin1(source: &[u8]) -> Vec<u8> {
  let mut latin1_bytes = Vec::with_capacity(source.len());
  let mut i = 0;
  while i < source.len() {
    match source[i] {
      byte if byte <= 0x7F => {
        // ASCII character
        latin1_bytes.push(byte);
        i += 1;
      }
      byte if (0xC2..=0xDF).contains(&byte) && i + 1 < source.len() => {
        // 2-byte UTF-8 sequence
        let codepoint =
          ((byte as u16 & 0x1F) << 6) | (source[i + 1] as u16 & 0x3F);
        latin1_bytes.push(if codepoint <= 0xFF {
          codepoint as u8
        } else {
          b'?'
        });
        i += 2;
      }
      _ => {
        // 3-byte or 4-byte UTF-8 sequence, or invalid UTF-8
        latin1_bytes.push(b'?');
        // Skip to the next valid UTF-8 start byte
        i += 1;
        while i < source.len() && (source[i] & 0xC0) == 0x80 {
          i += 1;
        }
      }
    }
  }
  latin1_bytes
}

fn utf8_to_ascii(source: &[u8]) -> Vec<u8> {
  let mut ascii_bytes = Vec::with_capacity(source.len());
  let mut i = 0;
  while i < source.len() {
    match source[i] {
      byte if byte <= 0x7F => {
        // ASCII character
        ascii_bytes.push(byte);
        i += 1;
      }
      _ => {
        // Non-ASCII character
        ascii_bytes.push(b'?');
        // Skip to the next valid UTF-8 start byte
        i += 1;
        while i < source.len() && (source[i] & 0xC0) == 0x80 {
          i += 1;
        }
      }
    }
  }
  ascii_bytes
}

// #[op2]
// pub fn op_node_decode_utf8<'a>(
//   scope: &mut v8::HandleScope<'a>,
//   #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
//   // buf: v8::Local<'a, v8::ArrayBuffer>,
//   // #[number] byte_offset: usize,
//   // #[number] byte_length: usize,
//   // #[number] start: Option<usize>,
//   // #[number] end: Option<usize>,
// ) -> Result<v8::Local<'a, v8::String>, JsErrorBox> {
//   let Some(args) = args else {
//     return Err(JsErrorBox::generic("Invalid arguments"));
//   };
//   let buf = args
//     .this()
//     .try_cast::<v8::Uint8Array>()
//     .map_err(|_| JsErrorBox::not_supported())?;
//   let byte_offset = buf.byte_offset();
//   let byte_length = buf.byte_length();

//   let zero_copy = {
//     let store = buf
//       .get_backing_store()
//       .ok_or_else(|| JsErrorBox::generic("Invalid buffer"))?;
//     unsafe {
//       deno_core::serde_v8::V8Slice::from_parts(
//         store,
//         byte_offset..byte_offset + byte_length,
//       )
//     }
//   };
//   let start = args.get(0);
//   let end = args.get(1);
//   let start = if start.is_null_or_undefined() {
//     0usize
//   } else {
//     start
//       .to_uint32(scope)
//       .ok_or_else(|| JsErrorBox::generic("Invalid start"))?
//       .value() as usize
//   };
//   let end = if end.is_null_or_undefined() {
//     byte_length as usize
//   } else {
//     end
//       .to_uint32(scope)
//       .ok_or_else(|| JsErrorBox::generic("Invalid end"))?
//       .value() as usize
//   };
//   // let start = start.unwrap_or(0);
//   // let end = end.unwrap_or(zero_copy.len());
//   if start > end {
//     return Err(JsErrorBox::generic("Invalid start and end"));
//   } else if end > zero_copy.len() {
//     return Err(JsErrorBox::generic("Invalid end"));
//   }
//   v8::String::new_from_utf8(
//     scope,
//     &zero_copy[start..end],
//     v8::NewStringType::Normal,
//   )
//   .ok_or_else(|| JsErrorBox::generic("Invalid UTF-8 sequence"))
// }

// #[op2]
// pub fn op_node_decode_utf8<'a>(
//   #[this] this: v8::Global<v8::Object>,
//   scope: &mut v8::HandleScope<'a>,
//   // buf: v8::Local<'a, v8::ArrayBuffer>,
//   // #[number] byte_offset: usize,
//   // #[number] byte_length: usize,
//   #[number] start: Option<usize>,
//   #[number] end: Option<usize>,
// ) -> Result<v8::Local<'a, v8::String>, JsErrorBox> {
//   let this = v8::Local::new(scope, this);
//   let buf = this
//     .try_cast::<v8::Uint8Array>()
//     .map_err(|_| JsErrorBox::not_supported())?;
//   let byte_offset = buf.byte_offset();
//   let byte_length = buf.byte_length();

//   let zero_copy = {
//     let store = buf
//       .get_backing_store()
//       .ok_or_else(|| JsErrorBox::generic("Invalid buffer"))?;
//     unsafe {
//       deno_core::serde_v8::V8Slice::from_parts(
//         store,
//         byte_offset..byte_offset + byte_length,
//       )
//     }
//   };
//   let start = start.unwrap_or(0);
//   let end = end.unwrap_or(byte_length as usize);
//   if start > end {
//     return Err(JsErrorBox::generic("Invalid start and end"));
//   } else if end > zero_copy.len() {
//     return Err(JsErrorBox::generic("Invalid end"));
//   }
//   v8::String::new_from_utf8(
//     scope,
//     &zero_copy[start..end],
//     v8::NewStringType::Normal,
//   )
//   .ok_or_else(|| JsErrorBox::generic("Invalid UTF-8 sequence"))
// }

// #[op2(no_side_effects)]
// pub fn op_node_decode_utf8<'a>(
//   scope: &mut v8::HandleScope<'a>,
//   buf: v8::Local<'a, v8::ArrayBuffer>,
//   #[number] byte_offset: usize,
//   #[number] byte_length: usize,
//   #[number] start: Option<usize>,
//   #[number] end: Option<usize>,
// ) -> Result<v8::Local<'a, v8::String>, JsErrorBox> {
//   let zero_copy = {
//     let store = buf.get_backing_store();
//     unsafe {
//       deno_core::serde_v8::V8Slice::from_parts(
//         store,
//         byte_offset..byte_offset + byte_length,
//       )
//     }
//   };
//   let start = start.unwrap_or(0);
//   let end = end.unwrap_or(byte_length as usize);
//   if start > end {
//     return Err(JsErrorBox::generic("Invalid start and end"));
//   } else if end > zero_copy.len() {
//     return Err(JsErrorBox::generic("Invalid end"));
//   }
//   v8::String::new_from_utf8(
//     scope,
//     &zero_copy[start..end],
//     v8::NewStringType::Normal,
//   )
//   .ok_or_else(|| JsErrorBox::generic("Invalid UTF-8 sequence"))
// }

// Recursive expansion of op2 macro
// =================================

#[allow(non_camel_case_types)]
pub const fn op_node_decode_utf8() -> ::deno_core::_ops::OpDecl {
  #[allow(non_camel_case_types)]
  pub struct op_node_decode_utf8 {
    _unconstructable: ::std::marker::PhantomData<()>,
  }
  impl ::deno_core::_ops::Op for op_node_decode_utf8 {
    const NAME: &'static str = "op_node_decode_utf8";
    const DECL: ::deno_core::_ops::OpDecl =
      ::deno_core::_ops::OpDecl::new_internal_op2(
        {
          const LITERAL: &'static [u8] = "op_node_decode_utf8".as_bytes();
          const STR: deno_core::v8::OneByteConst =
            deno_core::FastStaticString::create_external_onebyte_const(LITERAL);
          let s: &'static deno_core::v8::OneByteConst = &STR;
          ("op_node_decode_utf8", deno_core::FastStaticString::new(s))
        },
        false,
        false,
        false,
        6usize as u8,
        true,
        Self::v8_fn_ptr as _,
        Self::v8_fn_ptr_metrics as _,
        ::deno_core::AccessorType::None,
        None,
        None,
        ::deno_core::OpMetadata {
          ..::deno_core::OpMetadata::default()
        },
      );
  }
  impl op_node_decode_utf8 {
    pub const fn name() -> &'static str {
      <Self as deno_core::_ops::Op>::NAME
    }
    fn slow_function_impl<'s>(
      info: &'s deno_core::v8::FunctionCallbackInfo,
    ) -> usize {
      #[cfg(debug_assertions)]
      let _reentrancy_check_guard =
        deno_core::_ops::reentrancy_check(&<Self as deno_core::_ops::Op>::DECL);
      let mut scope = unsafe { deno_core::v8::CallbackScope::new(info) };
      let mut rv =
        deno_core::v8::ReturnValue::from_function_callback_info(info);
      let args =
        deno_core::v8::FunctionCallbackArguments::from_function_callback_info(
          info,
        );
      let result = {
        let arg1 = args.get(0usize as i32);
        // let Ok(mut arg1) =
        //   deno_core::_ops::v8_try_convert::<deno_core::v8::ArrayBuffer>(arg1)
        // else {
        //   deno_core::_ops::throw_error_one_byte_info(
        //     &info,
        //     "expected ArrayBuffer",
        //   );
        //   return 1;
        // };
        let arg1 = unsafe {
          std::mem::transmute::<v8::Local<v8::Value>, v8::Local<v8::ArrayBuffer>>(
            arg1,
          )
        };
        let arg1 = arg1;
        let arg2 = args.get(1usize as i32);
        let Some(arg2) = to_f64_option(&arg2) else {
          deno_core::_ops::throw_error_one_byte_info(&info, "expected f64");
          return 1;
        };
        let arg2 = arg2 as _;
        let arg3 = args.get(2usize as i32);
        let Some(arg3) = to_f64_option(&arg3) else {
          deno_core::_ops::throw_error_one_byte_info(&info, "expected f64");
          return 1;
        };
        let arg3 = arg3 as _;
        let arg4 = args.get(3usize as i32);
        let Some(arg4) = to_f64_option(&arg4) else {
          deno_core::_ops::throw_error_one_byte_info(&info, "expected f64");
          return 1;
        };
        let arg4 = arg4 as _;
        let arg5 = args.get(4usize as i32);
        let Some(arg5) = to_f64_option(&arg5) else {
          deno_core::_ops::throw_error_one_byte_info(&info, "expected f64");
          return 1;
        };
        let arg5 = arg5 as _;
        let arg0 = &mut scope;
        Self::call(arg0, arg1, arg2, arg3, arg4, arg5)
      };
      match result {
        Ok(result) => rv.set(deno_core::_ops::RustToV8NoScope::to_v8(result)),
        Err(err) => {
          let exception = deno_core::error::to_v8_error(&mut scope, &err);
          scope.throw_exception(exception);
          return 1;
        }
      };
      return 0;
    }
    extern "C" fn v8_fn_ptr<'s>(
      info: *const deno_core::v8::FunctionCallbackInfo,
    ) {
      let info: &'s _ = unsafe { &*info };
      Self::slow_function_impl(info);
    }
    extern "C" fn v8_fn_ptr_metrics<'s>(
      info: *const deno_core::v8::FunctionCallbackInfo,
    ) {
      let info: &'s _ = unsafe { &*info };
      let args =
        deno_core::v8::FunctionCallbackArguments::from_function_callback_info(
          info,
        );
      let opctx: &'s _ = unsafe {
        &*(deno_core::v8::Local::<deno_core::v8::External>::cast_unchecked(
          args.data(),
        )
        .value() as *const deno_core::_ops::OpCtx)
      };
      deno_core::_ops::dispatch_metrics_slow(
        opctx,
        deno_core::_ops::OpMetricsEvent::Dispatched,
      );
      let res = Self::slow_function_impl(info);
      if res == 0 {
        deno_core::_ops::dispatch_metrics_slow(
          opctx,
          deno_core::_ops::OpMetricsEvent::Completed,
        );
      } else {
        deno_core::_ops::dispatch_metrics_slow(
          opctx,
          deno_core::_ops::OpMetricsEvent::Error,
        );
      }
    }
  }
  impl op_node_decode_utf8 {
    #[allow(clippy::too_many_arguments)]
    pub fn call<'a>(
      scope: &mut v8::HandleScope<'a>,
      buf: v8::Local<'a, v8::ArrayBuffer>,
      byte_offset: usize,
      byte_length: usize,
      start: usize,
      end: usize,
    ) -> Result<v8::Local<'a, v8::String>, JsErrorBox> {
      let zero_copy = buffer_to_slice(&buf, byte_offset, byte_length);
      // eprintln!("start: {}, end: {}", start, end);
      if start > end {
        return Err(JsErrorBox::generic("Invalid start and end"));
      } else if end > byte_length {
        return Err(JsErrorBox::generic("Invalid end"));
      }
      let zero_copy = &zero_copy[start..end];

      if zero_copy.len() <= 256 && zero_copy.is_ascii() {
        v8::String::new_from_one_byte(
          scope,
          zero_copy,
          v8::NewStringType::Normal,
        )
        .ok_or_else(|| JsErrorBox::generic("Invalid ASCII sequence"))
      } else {
        v8::String::new_from_utf8(scope, &zero_copy, v8::NewStringType::Normal)
          .ok_or_else(|| JsErrorBox::generic("Invalid UTF-8 sequence"))
      }
    }
  }
  <op_node_decode_utf8 as ::deno_core::_ops::Op>::DECL
}

fn to_f64_option(arg: &v8::Local<v8::Value>) -> Option<f64> {
  // if arg.is_number() {
  // let arg = arg.to_number(scope).unwrap().value();
  let arg = unsafe {
    std::mem::transmute::<v8::Local<v8::Value>, v8::Local<v8::Number>>(*arg)
  };
  Some(arg.value())
  // } else {
  //   None
  // }
}

fn buffer_to_slice<'a>(
  buf: &'a v8::Local<v8::ArrayBuffer>,
  byte_offset: usize,
  byte_length: usize,
) -> &'a [u8] {
  let Some(ptr) = buf.data() else {
    return &[];
  };
  unsafe {
    let ptr = ptr.cast::<u8>().add(byte_offset);
    std::slice::from_raw_parts(ptr.as_ptr(), byte_length)
  }
}
