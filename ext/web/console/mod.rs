// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust implementation of the console/inspect machinery that used to live
//! in `ext/web/01_console.js`. The JS file is now a thin shim over the ops
//! and the cppgc-wrapped `Console` class defined here.

mod css;
mod error_fmt;
mod inspect;
mod preview;
mod quote;
mod table;
mod width;

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use inspect::*;
pub use preview::op_preview_entries;

const DEFAULT_INDENT: &str = "  "; // Default indent string

const STR_ABBREVIATE_SIZE: f64 = 10_000.0;

/// Marker symbol set by the JS shim on its `stylizeNoColor` and
/// `createStylizeWithColor(...)` functions so the engine can recognize them
/// and run the lookup natively.
const STYLIZE_MARKER: &str = "Deno.privateConsoleStylize";

// ---------------------------------------------------------------------------
// intrinsics

#[derive(Default)]
struct IntrinsicsSlot(Option<std::rc::Rc<CachedIntrinsics>>);

/// Parse-and-cache the intrinsics object (one-time per runtime). The OpState
/// borrow is scoped: the engine re-enters JS (which may need OpState, e.g.
/// for stack-trace source mapping), so no borrow may be held across the call.
fn cached_intrinsics<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  state: &std::rc::Rc<RefCell<OpState>>,
  obj: v8::Local<'s, v8::Object>,
) -> std::rc::Rc<CachedIntrinsics> {
  {
    let state = state.borrow();
    if let Some(slot) = state.try_borrow::<IntrinsicsSlot>() {
      if let Some(cached) = &slot.0 {
        if cached.matches(scope, obj) {
          return cached.clone();
        }
      }
    }
  }
  let parsed = std::rc::Rc::new(parse_intrinsics_global(scope, obj));
  state.borrow_mut().put(IntrinsicsSlot(Some(parsed.clone())));
  parsed
}

fn parse_intrinsics_global<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> CachedIntrinsics {
  fn get_fn<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    obj: v8::Local<'s, v8::Object>,
    key: &'static str,
  ) -> v8::Global<v8::Function> {
    let v = try_get_str(scope, obj, key)
      .unwrap_or_else(|| v8::undefined(scope).into());
    let f = v8::Local::<v8::Function>::try_from(v)
      .unwrap_or_else(|_| panic!("intrinsics.{key} must be a function"));
    v8::Global::new(scope, f)
  }
  fn get_obj<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    obj: v8::Local<'s, v8::Object>,
    key: &'static str,
  ) -> v8::Global<v8::Object> {
    let v = try_get_str(scope, obj, key)
      .unwrap_or_else(|| v8::undefined(scope).into());
    let o = v8::Local::<v8::Object>::try_from(v)
      .unwrap_or_else(|_| panic!("intrinsics.{key} must be an object"));
    v8::Global::new(scope, o)
  }

  // wellKnown: flat array of [proto, name, constructor, ...] triples.
  let mut well_known = Vec::new();
  let wk = try_get_str(scope, obj, "wellKnown")
    .and_then(|v| v8::Local::<v8::Array>::try_from(v).ok());
  if let Some(wk) = wk {
    let len = wk.length();
    let mut i = 0u32;
    while i + 3 <= len {
      let proto = wk
        .get_index(scope, i)
        .and_then(|v| v8::Local::<v8::Object>::try_from(v).ok());
      let name = wk
        .get_index(scope, i + 1)
        .map(|v| v.to_rust_string_lossy(scope));
      let ctor = wk
        .get_index(scope, i + 2)
        .and_then(|v| v8::Local::<v8::Function>::try_from(v).ok());
      if let (Some(proto), Some(name), Some(ctor)) = (proto, name, ctor) {
        well_known.push((
          v8::Global::new(scope, proto),
          name,
          v8::Global::new(scope, ctor),
        ));
      }
      i += 3;
    }
  }

  CachedIntrinsics {
    key: v8::Global::new(scope, obj),
    function_to_string: get_fn(scope, obj, "functionToString"),
    inspect_fn: get_fn(scope, obj, "inspect"),
    stylize_no_color: get_fn(scope, obj, "stylizeNoColor"),
    create_stylize_with_color: get_fn(scope, obj, "createStylizeWithColor"),
    styles_obj: get_obj(scope, obj, "styles"),
    colors_obj: get_obj(scope, obj, "colors"),
    object_prototype: get_obj(scope, obj, "objectPrototype"),
    error_prototype: get_obj(scope, obj, "errorPrototype"),
    well_known,
    get_url_prototype: get_fn(scope, obj, "getURLPrototype"),
    get_cwd: get_fn(scope, obj, "getCwd"),
    make_cross_context_stylize: get_fn(scope, obj, "makeCrossContextStylize"),
    reg_exp_to_string: get_fn(scope, obj, "regExpToString"),
    number_value_of: get_fn(scope, obj, "numberValueOf"),
    string_value_of: get_fn(scope, obj, "stringValueOf"),
    boolean_value_of: get_fn(scope, obj, "booleanValueOf"),
    big_int_value_of: get_fn(scope, obj, "bigIntValueOf"),
    symbol_value_of: get_fn(scope, obj, "symbolValueOf"),
  }
}

// ---------------------------------------------------------------------------
// option parsing

fn detect_stylize<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  intr: &Intrinsics<'_>,
  value: v8::Local<'s, v8::Value>,
) -> Option<StylizeKind<'s>> {
  let f = v8::Local::<v8::Function>::try_from(value).ok()?;
  let marker = symbol_for(scope, STYLIZE_MARKER);
  let f_obj: v8::Local<v8::Object> = f.into();
  let payload = try_get(scope, f_obj, marker.into());
  match payload {
    Some(p) if p.is_string() => {
      // "noColor"
      Some(StylizeKind::NoColor)
    }
    Some(p) if p.is_array() => {
      let arr = v8::Local::<v8::Array>::try_from(p).ok()?;
      let styles = arr
        .get_index(scope, 0)
        .and_then(|v| v8::Local::<v8::Object>::try_from(v).ok())?;
      let colors = arr
        .get_index(scope, 1)
        .and_then(|v| v8::Local::<v8::Object>::try_from(v).ok())?;
      Some(StylizeKind::Theme {
        styles,
        colors,
        js_fn: Some(f),
      })
    }
    _ => {
      let _ = intr;
      Some(StylizeKind::Js(f))
    }
  }
}

fn default_ctx<'s>() -> Ctx<'s> {
  Ctx {
    show_hidden: false,
    depth: Some(4.0),
    colors: false,
    custom_inspect: true,
    show_proxy: false,
    max_array_length: 100.0,
    max_string_length: 10_000.0,
    break_length: 80.0,
    escape_sequences: true,
    compact: Compact::Num(3.0),
    sorted: Sorted::No,
    getters: Getters::No,
    quotes: vec!["\"".to_string(), "'".to_string(), "`".to_string()],
    iterable_limit: 100.0,
    trailing_comma: false,
    indent_level: 0.0,
    str_abbreviate_size: None,
    indentation_lvl: 0,
    current_depth: 0.0,
    seen: Vec::new(),
    circular: Vec::new(),
    circular_set: false,
    budget: HashMap::new(),
    stylize: StylizeKind::NoColor,
    user_options: None,
    numeric_separator: None,
    ctx_inspect_fn: None,
    url_prototype: None,
    circular_error_message: None,
    stylize_js_fn: None,
    theme_memo: HashMap::new(),
  }
}

fn opt_num<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: &'static str,
) -> Option<v8::Local<'s, v8::Value>> {
  try_get_str(scope, obj, key).filter(|v| !v.is_undefined())
}

/// `null` maps to Infinity for maxArrayLength/maxStringLength.
fn read_limit<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  v: v8::Local<'s, v8::Value>,
) -> f64 {
  if v.is_null() {
    f64::INFINITY
  } else {
    v.number_value(scope).unwrap_or(f64::NAN)
  }
}

/// Apply an options-bag object on top of a ctx (mirrors object spread).
/// Iterates the object's own enumerable keys once instead of probing every
/// known option (callers often pass small or empty bags).
fn apply_options<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  options: v8::Local<'s, v8::Object>,
) {
  let names = {
    v8::tc_scope!(tc, scope);
    options.get_property_names(
      tc,
      v8::GetPropertyNamesArgs {
        mode: v8::KeyCollectionMode::OwnOnly,
        property_filter: v8::PropertyFilter::ONLY_ENUMERABLE
          | v8::PropertyFilter::SKIP_SYMBOLS,
        index_filter: v8::IndexFilter::IncludeIndices,
        key_conversion: v8::KeyConversionMode::ConvertToString,
      },
    )
  };
  let Some(names) = names else {
    return;
  };
  for i in 0..names.length() {
    let Some(key) = names.get_index(scope, i) else {
      continue;
    };
    let key_str = key.to_rust_string_lossy(scope);
    let Some(v) = try_get(scope, options, key) else {
      continue;
    };
    if v.is_undefined() && key_str != "depth" {
      continue;
    }
    match key_str.as_str() {
      "showHidden" => ctx.show_hidden = v.boolean_value(scope),
      "depth" => {
        if v.is_undefined() {
          continue;
        }
        ctx.depth = if v.is_null() {
          None
        } else {
          Some(v.number_value(scope).unwrap_or(f64::NAN))
        };
      }
      "colors" => ctx.colors = v.boolean_value(scope),
      "customInspect" => ctx.custom_inspect = v.boolean_value(scope),
      "showProxy" => ctx.show_proxy = v.boolean_value(scope),
      "maxArrayLength" => ctx.max_array_length = read_limit(scope, v),
      "maxStringLength" => ctx.max_string_length = read_limit(scope, v),
      "breakLength" => {
        ctx.break_length = v.number_value(scope).unwrap_or(f64::NAN);
      }
      "escapeSequences" => ctx.escape_sequences = v.boolean_value(scope),
      "compact" => {
        ctx.compact = if v.is_number() {
          Compact::Num(v.number_value(scope).unwrap_or(0.0))
        } else {
          Compact::Bool(v.is_true())
        };
      }
      "sorted" => {
        ctx.sorted = if let Ok(f) = v8::Local::<v8::Function>::try_from(v) {
          Sorted::Comparator(f)
        } else if v.boolean_value(scope) {
          Sorted::Yes
        } else {
          Sorted::No
        };
      }
      "getters" => {
        ctx.getters = if v.is_string() {
          match v.to_rust_string_lossy(scope).as_str() {
            "get" => Getters::Get,
            "set" => Getters::Set,
            _ => Getters::No,
          }
        } else if v.boolean_value(scope) {
          Getters::Yes
        } else {
          Getters::No
        };
      }
      "quotes" => {
        if let Ok(arr) = v8::Local::<v8::Array>::try_from(v) {
          let mut quotes = Vec::with_capacity(arr.length() as usize);
          for i in 0..arr.length() {
            if let Some(q) = arr.get_index(scope, i) {
              quotes.push(q.to_rust_string_lossy(scope));
            }
          }
          ctx.quotes = quotes;
        }
      }
      "iterableLimit" => {
        ctx.iterable_limit = v.number_value(scope).unwrap_or(f64::NAN);
      }
      "trailingComma" => ctx.trailing_comma = v.boolean_value(scope),
      "indentLevel" => {
        ctx.indent_level = v.number_value(scope).unwrap_or(0.0);
      }
      "strAbbreviateSize" => {
        ctx.str_abbreviate_size =
          Some(v.number_value(scope).unwrap_or(f64::NAN));
      }
      "indentationLvl" => {
        ctx.indentation_lvl =
          v.number_value(scope).unwrap_or(0.0).max(0.0) as usize;
      }
      "numericSeparator" => ctx.numeric_separator = Some(v),
      "userOptions" => {
        ctx.user_options = v8::Local::<v8::Object>::try_from(v).ok();
      }
      "inspect" => ctx.ctx_inspect_fn = Some(v),
      "stylize" => {
        if let Some(kind) = detect_stylize(scope, intr, v) {
          ctx.stylize = kind;
          ctx.stylize_js_fn = v8::Local::<v8::Function>::try_from(v).ok();
        }
      }
      "seen" => {
        // Shared seen array (custom-inspect hooks pass the live ctx back).
        if let Ok(arr) = v8::Local::<v8::Array>::try_from(v) {
          let mut seen = Vec::with_capacity(arr.length() as usize);
          for i in 0..arr.length() {
            if let Some(item) = arr.get_index(scope, i) {
              seen.push(item);
            }
          }
          ctx.seen = seen;
        }
      }
      "budget" => {
        if let Ok(budget_obj) = v8::Local::<v8::Object>::try_from(v) {
          v8::tc_scope!(tc, scope);
          if let Some(names) =
            budget_obj.get_own_property_names(tc, Default::default())
          {
            for i in 0..names.length() {
              let Some(key) = names.get_index(tc, i) else {
                continue;
              };
              let key_str = key.to_rust_string_lossy(tc);
              let Ok(lvl) = key_str.parse::<usize>() else {
                continue;
              };
              let Some(val) = budget_obj.get(tc, key) else {
                continue;
              };
              let n = val.number_value(tc).unwrap_or(0.0).max(0.0) as usize;
              ctx.budget.insert(lvl, n);
            }
          }
        }
      }
      _ => {}
    }
  }
}

/// Post-processing shared by `inspect()` and `inspectArgs()`.
fn finish_options<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  options: Option<v8::Local<'s, v8::Object>>,
) {
  if let Some(options) = options {
    if let Some(v) = opt_num(scope, options, "iterableLimit") {
      ctx.max_array_length = v.number_value(scope).unwrap_or(f64::NAN);
    }
    if let Some(v) = opt_num(scope, options, "strAbbreviateSize") {
      ctx.max_string_length = v.number_value(scope).unwrap_or(f64::NAN);
    }
  }
  if ctx.colors {
    ctx.stylize = StylizeKind::Theme {
      styles: intr.styles_obj(scope),
      colors: intr.colors_obj(scope),
      js_fn: None,
    };
    ctx.stylize_js_fn = None;
  }
  // maxArrayLength/maxStringLength null already mapped to Infinity.
}

// ---------------------------------------------------------------------------
// inspectArgs

fn js_string_char_at(chars: &[u16], i: usize) -> Option<u16> {
  chars.get(i).copied()
}

fn utf16_to_string(units: &[u16]) -> String {
  String::from_utf16_lossy(units)
}

/// `String(value)` semantics (symbols stringify instead of throwing).
fn js_string<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
) -> R<String> {
  if value.is_symbol() {
    let sym = v8::Local::<v8::Symbol>::try_from(value).unwrap();
    let desc = sym.description(scope);
    return Ok(if desc.is_undefined() {
      "Symbol()".to_string()
    } else {
      format!("Symbol({})", desc.to_rust_string_lossy(scope))
    });
  }
  v8::tc_scope!(tc, scope);
  match value.to_string(tc) {
    Some(s) => Ok(s.to_rust_string_lossy(tc)),
    None => {
      let exc = tc.exception();
      Err(grab_err(tc, exc))
    }
  }
}

/// `NumberParseInt(value)` then template-stringify.
fn js_parse_int_display<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
) -> R<String> {
  let s = js_string(scope, value)?;
  let n = js_parse_int(&s);
  Ok(js_number_to_string(scope, n))
}

fn js_parse_int(s: &str) -> f64 {
  let t = s.trim_start_matches(|c: char| c.is_whitespace());
  let (sign, rest) = match t.strip_prefix('-') {
    Some(r) => (-1.0, r),
    None => (1.0, t.strip_prefix('+').unwrap_or(t)),
  };
  let (radix, digits) = if let Some(hex) =
    rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X"))
  {
    (16u32, hex)
  } else {
    (10u32, rest)
  };
  let mut value: f64 = f64::NAN;
  let mut acc = 0.0f64;
  let mut any = false;
  for c in digits.chars() {
    match c.to_digit(radix) {
      Some(d) => {
        acc = acc * radix as f64 + d as f64;
        any = true;
      }
      None => break,
    }
  }
  if any {
    value = sign * acc;
  }
  value
}

fn js_parse_float(s: &str) -> f64 {
  let t = s.trim_start_matches(|c: char| c.is_whitespace());
  // Longest prefix that parses as a float literal.
  let mut end = 0;
  let bytes = t.as_bytes();
  let mut i = 0;
  if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
    i += 1;
  }
  if t[i..].starts_with("Infinity") {
    return if bytes.first() == Some(&b'-') {
      f64::NEG_INFINITY
    } else {
      f64::INFINITY
    };
  }
  let mut seen_digit = false;
  while i < bytes.len() && bytes[i].is_ascii_digit() {
    i += 1;
    seen_digit = true;
  }
  if i < bytes.len() && bytes[i] == b'.' {
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
      i += 1;
      seen_digit = true;
    }
  }
  if seen_digit {
    end = i;
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
      let mut j = i + 1;
      if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
        j += 1;
      }
      let mut exp_digit = false;
      while j < bytes.len() && bytes[j].is_ascii_digit() {
        j += 1;
        exp_digit = true;
      }
      if exp_digit {
        end = j;
      }
    }
  }
  if end == 0 {
    return f64::NAN;
  }
  t[..end].parse::<f64>().unwrap_or(f64::NAN)
}

fn js_number_to_string<'s>(scope: &mut v8::PinScope<'s, '_>, n: f64) -> String {
  let num = v8::Number::new(scope, n);
  num.to_rust_string_lossy(scope)
}

/// `tryStringify(arg)`: JSON.stringify, mapping circular errors to
/// "[Circular]".
fn try_stringify<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
) -> R<String> {
  let result = {
    v8::tc_scope!(tc, scope);
    match v8::json::stringify(tc, value) {
      Some(s) => Ok(s.to_rust_string_lossy(tc)),
      None => {
        let exc = tc.exception();
        Err(grab_err(tc, exc))
      }
    }
  };
  match result {
    Ok(s) => Ok(s),
    Err(err) => {
      // Compute the canonical circular error message once.
      if ctx.circular_error_message.is_none() {
        let sample = v8::Object::new(scope);
        let key = v8_str(scope, "a");
        sample.set(scope, key.into(), sample.into());
        let msg = {
          v8::tc_scope!(tc, scope);
          match v8::json::stringify(tc, sample.into()) {
            Some(_) => String::new(),
            None => tc
              .exception()
              .and_then(|exc| {
                v8::Local::<v8::Object>::try_from(exc)
                  .ok()
                  .and_then(|o| {
                    let key = v8_str(tc, "message");
                    o.get(tc, key.into())
                  })
                  .map(|m| m.to_rust_string_lossy(tc))
              })
              .unwrap_or_default(),
          }
        };
        ctx.circular_error_message =
          Some(msg.split('\n').next().unwrap_or("").to_string());
      }
      // Only treat TypeErrors with the circular message as circular.
      let exc = v8::Local::new(scope, &err.0);
      let is_circular = (|| {
        let obj = v8::Local::<v8::Object>::try_from(exc).ok()?;
        let name = try_get_str(scope, obj, "name")?;
        if name.to_rust_string_lossy(scope) != "TypeError" {
          return None;
        }
        let message = try_get_str(scope, obj, "message")?;
        let message = message.to_rust_string_lossy(scope);
        let first_line = message.split('\n').next().unwrap_or("");
        if Some(first_line) == ctx.circular_error_message.as_deref() {
          Some(())
        } else {
          None
        }
      })()
      .is_some();
      if is_circular {
        Ok("[Circular]".to_string())
      } else {
        Err(err)
      }
    }
  }
}

#[allow(clippy::too_many_arguments, reason = "formatting context")]
fn inspect_args_impl<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  args: &[v8::Local<'s, v8::Value>],
) -> R<String> {
  let no_color = !ctx.colors;
  let mut a = 0usize;
  let mut string = String::new();

  if !args.is_empty() && args[0].is_string() && args.len() > 1 {
    let first_str = args[0].to_rust_string_lossy(scope);
    let first: Vec<u16> = first_str.encode_utf16().collect();
    a += 1;
    let mut appended_chars = 0usize;
    let mut used_style = false;
    let mut prev_css: Option<css::Css> = None;
    let mut i = 0usize;
    while i + 1 < first.len() {
      if js_string_char_at(&first, i) == Some(b'%' as u16) {
        i += 1;
        let ch = js_string_char_at(&first, i).unwrap_or(0);
        if a < args.len() {
          let mut formatted_arg: Option<String> = None;
          match ch as u8 as char {
            's' if ch < 128 => {
              formatted_arg = Some(js_string(scope, args[a])?);
              a += 1;
            }
            'd' | 'i' if ch < 128 => {
              let value = args[a];
              a += 1;
              formatted_arg = Some(if value.is_symbol() {
                "NaN".to_string()
              } else {
                js_parse_int_display(scope, value)?
              });
            }
            'f' if ch < 128 => {
              let value = args[a];
              a += 1;
              formatted_arg = Some(if value.is_symbol() {
                "NaN".to_string()
              } else {
                let s = js_string(scope, value)?;
                js_number_to_string(scope, js_parse_float(&s))
              });
            }
            'j' if ch < 128 => {
              let value = args[a];
              a += 1;
              formatted_arg = Some(try_stringify(scope, ctx, value)?);
            }
            'O' | 'o' if ch < 128 => {
              let value = args[a];
              a += 1;
              formatted_arg =
                Some(format_value(scope, intr, ctx, value, 0.0, false)?);
            }
            'c' if ch < 128 => {
              let value = args[a];
              a += 1;
              if !no_color {
                let css_str = js_string(scope, value)?;
                let parsed = css::parse_css(&css_str);
                let ansi = css::css_to_ansi(&parsed, prev_css.as_ref());
                if !ansi.is_empty() {
                  used_style = true;
                  prev_css = Some(parsed);
                }
                formatted_arg = Some(ansi);
              } else {
                formatted_arg = Some(String::new());
              }
            }
            _ => {}
          }
          if let Some(formatted) = formatted_arg {
            string.push_str(&utf16_to_string(&first[appended_chars..i - 1]));
            string.push_str(&formatted);
            appended_chars = i + 1;
          }
        }
        if ch == b'%' as u16 {
          string.push_str(&utf16_to_string(&first[appended_chars..i - 1]));
          string.push('%');
          appended_chars = i + 1;
        }
      }
      i += 1;
    }
    string.push_str(&utf16_to_string(&first[appended_chars..]));
    if used_style {
      string.push_str("\u{1b}[0m");
    }
  }

  while a < args.len() {
    if a > 0 {
      string.push(' ');
    }
    if args[a].is_string() {
      string.push_str(&args[a].to_rust_string_lossy(scope));
    } else {
      string.push_str(&format_value(scope, intr, ctx, args[a], 0.0, false)?);
    }
    a += 1;
  }

  if ctx.indent_level > 0.0 {
    let group_indent = DEFAULT_INDENT.repeat(ctx.indent_level as usize);
    string = format!(
      "{group_indent}{}",
      string.replace('\n', &format!("\n{group_indent}"))
    );
  }

  Ok(string)
}

// ---------------------------------------------------------------------------
// ops

fn rethrow<'s>(scope: &mut v8::PinScope<'s, '_>, err: JsErr) {
  let exc = v8::Local::new(scope, &err.0);
  scope.throw_exception(exc);
}

fn collect_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  arr: v8::Local<'s, v8::Array>,
) -> Vec<v8::Local<'s, v8::Value>> {
  let mut out = Vec::with_capacity(arr.length() as usize);
  for i in 0..arr.length() {
    out.push(
      arr
        .get_index(scope, i)
        .unwrap_or_else(|| v8::undefined(scope).into()),
    );
  }
  out
}

/// `inspectArgs(args, inspectOptions)`.
#[op2(reentrant)]
#[string]
pub fn op_console_inspect_args<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  state: std::rc::Rc<RefCell<OpState>>,
  intrinsics: v8::Local<'s, v8::Object>,
  args: v8::Local<'s, v8::Array>,
  options: v8::Local<'s, v8::Value>,
) -> Option<String> {
  let cached = cached_intrinsics(scope, &state, intrinsics);
  let intr = Intrinsics::new(&cached);
  let mut ctx = default_ctx();
  let options_obj = v8::Local::<v8::Object>::try_from(options).ok();
  if let Some(options_obj) = options_obj {
    apply_options(scope, &intr, &mut ctx, options_obj);
  }
  finish_options(scope, &intr, &mut ctx, options_obj);
  let args = collect_array(scope, args);
  match inspect_args_impl(scope, &intr, &mut ctx, &args) {
    Ok(s) => Some(s),
    Err(err) => {
      rethrow(scope, err);
      None
    }
  }
}

/// `inspect(value, inspectOptions)`.
#[op2(reentrant)]
#[string]
pub fn op_console_inspect<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  state: std::rc::Rc<RefCell<OpState>>,
  intrinsics: v8::Local<'s, v8::Object>,
  value: v8::Local<'s, v8::Value>,
  options: v8::Local<'s, v8::Value>,
) -> Option<String> {
  let cached = cached_intrinsics(scope, &state, intrinsics);
  let intr = Intrinsics::new(&cached);
  let mut ctx = default_ctx();
  let options_obj = v8::Local::<v8::Object>::try_from(options).ok();
  if let Some(options_obj) = options_obj {
    apply_options(scope, &intr, &mut ctx, options_obj);
  }
  finish_options(scope, &intr, &mut ctx, options_obj);
  match format_value(scope, &intr, &mut ctx, value, 0.0, false) {
    Ok(s) => Some(s),
    Err(err) => {
      rethrow(scope, err);
      None
    }
  }
}

/// `formatValue(ctx, value, recurseTimes)` — node `util.inspect` entry.
#[op2(reentrant)]
#[string]
pub fn op_console_format_value<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  state: std::rc::Rc<RefCell<OpState>>,
  intrinsics: v8::Local<'s, v8::Object>,
  ctx_obj: v8::Local<'s, v8::Value>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> Option<String> {
  let cached = cached_intrinsics(scope, &state, intrinsics);
  let intr = Intrinsics::new(&cached);
  let mut ctx = default_ctx();
  if let Ok(ctx_o) = v8::Local::<v8::Object>::try_from(ctx_obj) {
    apply_options(scope, &intr, &mut ctx, ctx_o);
  }
  // No finish_options: formatValue honors ctx.stylize as provided.
  match format_value(scope, &intr, &mut ctx, value, recurse_times, false) {
    Ok(s) => Some(s),
    Err(err) => {
      rethrow(scope, err);
      None
    }
  }
}

/// `quoteString(string, ctx)`.
#[op2]
#[string]
pub fn op_console_quote_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  #[string] string: String,
  ctx_obj: v8::Local<'s, v8::Value>,
) -> String {
  let mut quotes = vec!["\"".to_string(), "'".to_string(), "`".to_string()];
  let mut escape_sequences = true;
  if let Ok(ctx_o) = v8::Local::<v8::Object>::try_from(ctx_obj) {
    if let Some(v) = opt_num(scope, ctx_o, "quotes") {
      if let Ok(arr) = v8::Local::<v8::Array>::try_from(v) {
        quotes.clear();
        for i in 0..arr.length() {
          if let Some(q) = arr.get_index(scope, i) {
            quotes.push(q.to_rust_string_lossy(scope));
          }
        }
      }
    }
    if let Some(v) = opt_num(scope, ctx_o, "escapeSequences") {
      escape_sequences = v.boolean_value(scope);
    }
  }
  quote::quote_string(&string, &quotes, escape_sequences)
}

#[op2]
#[serde]
pub fn op_console_parse_css(#[string] css_string: String) -> css::Css {
  css::parse_css(&css_string)
}

#[op2]
#[serde]
pub fn op_console_parse_css_color(
  #[string] color_string: String,
) -> Option<[u8; 3]> {
  css::parse_css_color(&color_string)
}

#[op2]
#[string]
pub fn op_console_css_to_ansi(
  #[serde] css_obj: css::Css,
  #[serde] prev_css: Option<css::Css>,
) -> String {
  css::css_to_ansi(&css_obj, prev_css.as_ref())
}

#[op2(fast)]
pub fn op_console_get_string_width(
  #[string] s: &str,
  remove_control_chars: bool,
) -> u32 {
  width::get_string_width(s, remove_control_chars) as u32
}

#[op2]
#[string]
pub fn op_console_strip_vt(#[string] s: &str) -> String {
  width::strip_vt_control_characters(s)
}

// ---------------------------------------------------------------------------
// Console (cppgc object wrap)

pub struct Console {
  print_func: v8::TracedReference<v8::Function>,
  intrinsics: v8::TracedReference<v8::Object>,
  intr_cache: RefCell<Option<std::rc::Rc<CachedIntrinsics>>>,
  no_color_stdout: v8::TracedReference<v8::Function>,
  no_color_stderr: v8::TracedReference<v8::Function>,
  count_map: RefCell<HashMap<String, u64>>,
  timer_map: RefCell<HashMap<String, Instant>>,
  indent_level: Cell<f64>,
}

// SAFETY: all v8 references are traced.
unsafe impl GarbageCollected for Console {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.print_func);
    visitor.trace(&self.intrinsics);
    visitor.trace(&self.no_color_stdout);
    visitor.trace(&self.no_color_stderr);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Console"
  }
}

impl Console {
  fn no_color<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    stderr: bool,
  ) -> bool {
    let f = if stderr {
      self.no_color_stderr.get(scope)
    } else {
      self.no_color_stdout.get(scope)
    };
    let Some(f) = f else {
      return false;
    };
    let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
    match js_call(scope, f, undef, &[]) {
      Ok(v) => v.boolean_value(scope),
      Err(_) => false,
    }
  }

  fn print<'s>(&self, scope: &mut v8::PinScope<'s, '_>, msg: &str, level: i32) {
    let Some(f) = self.print_func.get(scope) else {
      return;
    };
    let msg = v8_str(scope, msg);
    let level = v8::Integer::new(scope, level);
    let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
    let _ = js_call(scope, f, undef, &[msg.into(), level.into()]);
  }

  /// `getConsoleInspectOptions(noColor)`-equivalent ctx.
  fn console_ctx<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    intr: &Intrinsics<'_>,
    stderr: bool,
  ) -> Ctx<'s> {
    let no_color = self.no_color(scope, stderr);
    let mut ctx = default_ctx();
    ctx.colors = !no_color;
    ctx.indent_level = self.indent_level.get();
    if ctx.colors {
      ctx.stylize = StylizeKind::Theme {
        styles: intr.styles_obj(scope),
        colors: intr.colors_obj(scope),
        js_fn: None,
      };
    }
    ctx
  }

  fn intr<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> Option<std::rc::Rc<CachedIntrinsics>> {
    let obj = self.intrinsics.get(scope)?;
    if let Some(cached) = self.intr_cache.borrow().as_ref() {
      if cached.matches(scope, obj) {
        return Some(cached.clone());
      }
    }
    let parsed = std::rc::Rc::new(parse_intrinsics_global(scope, obj));
    *self.intr_cache.borrow_mut() = Some(parsed.clone());
    Some(parsed)
  }

  fn log_with_level<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    args: Option<&v8::FunctionCallbackArguments<'a>>,
    stderr: bool,
    level: i32,
  ) {
    let Some(cached) = self.intr(scope) else {
      return;
    };
    let intr = Intrinsics::new(&cached);
    let mut ctx = self.console_ctx(scope, &intr, stderr);
    let args: Vec<v8::Local<v8::Value>> = match args {
      Some(args) => (0..args.length()).map(|i| args.get(i)).collect(),
      None => Vec::new(),
    };
    match inspect_args_impl(scope, &intr, &mut ctx, &args) {
      Ok(s) => self.print(scope, &format!("{s}\n"), level),
      Err(err) => rethrow(scope, err),
    }
  }

  fn duration_display(duration_ms: f64) -> String {
    if duration_ms < 1.0 {
      format!("{duration_ms:.3}")
    } else if duration_ms < 10.0 {
      format!("{duration_ms:.2}")
    } else if duration_ms < 100.0 {
      format!("{duration_ms:.1}")
    } else {
      format!("{}", duration_ms.round() as i64)
    }
  }
}

#[op2]
impl Console {
  #[constructor]
  #[cppgc]
  fn constructor<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    print_func: v8::Local<'a, v8::Function>,
    intrinsics: v8::Local<'a, v8::Object>,
    no_color_stdout: v8::Local<'a, v8::Function>,
    no_color_stderr: v8::Local<'a, v8::Function>,
  ) -> Console {
    Console {
      print_func: v8::TracedReference::new(scope, print_func),
      intrinsics: v8::TracedReference::new(scope, intrinsics),
      intr_cache: RefCell::new(None),
      no_color_stdout: v8::TracedReference::new(scope, no_color_stdout),
      no_color_stderr: v8::TracedReference::new(scope, no_color_stderr),
      count_map: RefCell::new(HashMap::new()),
      timer_map: RefCell::new(HashMap::new()),
      indent_level: Cell::new(0.0),
    }
  }

  #[getter]
  fn indent_level(&self) -> f64 {
    self.indent_level.get()
  }

  #[setter]
  fn indent_level(&self, value: f64) {
    self.indent_level.set(value);
  }

  #[nofast]
  #[reentrant]
  fn log<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) {
    self.log_with_level(scope, args, false, 1);
  }

  #[nofast]
  #[reentrant]
  fn debug<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) {
    self.log_with_level(scope, args, false, 0);
  }

  #[nofast]
  #[reentrant]
  fn info<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) {
    self.log_with_level(scope, args, false, 1);
  }

  #[nofast]
  #[reentrant]
  fn warn<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) {
    self.log_with_level(scope, args, true, 2);
  }

  #[nofast]
  #[reentrant]
  fn error<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) {
    self.log_with_level(scope, args, true, 3);
  }

  #[nofast]
  #[reentrant]
  fn dir<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    obj: v8::Local<'a, v8::Value>,
    options: v8::Local<'a, v8::Value>,
  ) {
    let Some(cached) = self.intr(scope) else {
      return;
    };
    let intr = Intrinsics::new(&cached);
    let mut ctx = self.console_ctx(scope, &intr, false);
    let options_obj = v8::Local::<v8::Object>::try_from(options).ok();
    if let Some(options_obj) = options_obj {
      apply_options(scope, &intr, &mut ctx, options_obj);
      finish_options(scope, &intr, &mut ctx, Some(options_obj));
    }
    match inspect_args_impl(scope, &intr, &mut ctx, &[obj]) {
      Ok(s) => self.print(scope, &format!("{s}\n"), 1),
      Err(err) => rethrow(scope, err),
    }
  }

  #[nofast]
  #[reentrant]
  fn assert<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    condition: bool,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) {
    if condition {
      return;
    }
    let Some(cached) = self.intr(scope) else {
      return;
    };
    let intr = Intrinsics::new(&cached);
    let mut ctx = self.console_ctx(scope, &intr, true);
    let rest: Vec<v8::Local<v8::Value>> = match args {
      // First vararg is the condition itself; skip it.
      Some(args) => (1..args.length()).map(|i| args.get(i)).collect(),
      None => Vec::new(),
    };
    let mut final_args: Vec<v8::Local<v8::Value>> = Vec::new();
    if rest.is_empty() {
      let msg = v8_str(scope, "Assertion failed");
      final_args.push(msg.into());
    } else if rest[0].is_string() {
      let first = rest[0].to_rust_string_lossy(scope);
      let msg = v8_str(scope, &format!("Assertion failed: {first}"));
      final_args.push(msg.into());
      final_args.extend_from_slice(&rest[1..]);
    } else {
      let msg = v8_str(scope, "Assertion failed:");
      final_args.push(msg.into());
      final_args.extend_from_slice(&rest);
    }
    match inspect_args_impl(scope, &intr, &mut ctx, &final_args) {
      Ok(s) => self.print(scope, &format!("{s}\n"), 3),
      Err(err) => rethrow(scope, err),
    }
  }

  #[nofast]
  #[reentrant]
  fn count(&self, scope: &mut v8::PinScope<'_, '_>, #[string] label: String) {
    let mut map = self.count_map.borrow_mut();
    let entry = map.entry(label.clone()).or_insert(0);
    *entry += 1;
    let current = *entry;
    drop(map);
    self.print(scope, &format!("{label}: {current}\n"), 1);
  }

  #[nofast]
  #[reentrant]
  fn count_reset(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[string] label: String,
  ) {
    let mut map = self.count_map.borrow_mut();
    if map.contains_key(&label) {
      map.insert(label, 0);
      return;
    }
    drop(map);
    self.print(scope, &format!("Count for '{label}' does not exist\n"), 2);
  }

  #[nofast]
  #[reentrant]
  fn time(&self, scope: &mut v8::PinScope<'_, '_>, #[string] label: String) {
    let mut map = self.timer_map.borrow_mut();
    if map.contains_key(&label) {
      drop(map);
      self.print(scope, &format!("Timer '{label}' already exists\n"), 2);
      return;
    }
    map.insert(label, Instant::now());
  }

  #[nofast]
  #[reentrant]
  fn time_log<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[string] label: String,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) {
    let start = self.timer_map.borrow().get(&label).copied();
    let Some(start) = start else {
      self.print(scope, &format!("Timer '{label}' does not exist\n"), 2);
      return;
    };
    let duration = start.elapsed().as_secs_f64() * 1000.0;
    let display = Self::duration_display(duration);
    let Some(cached) = self.intr(scope) else {
      return;
    };
    let intr = Intrinsics::new(&cached);
    let mut ctx = self.console_ctx(scope, &intr, false);
    let label_msg = v8_str(scope, &format!("{label}: {display}ms"));
    let mut final_args: Vec<v8::Local<v8::Value>> = vec![label_msg.into()];
    if let Some(args) = args {
      // First vararg is the label; skip it.
      for i in 1..args.length() {
        final_args.push(args.get(i));
      }
    }
    match inspect_args_impl(scope, &intr, &mut ctx, &final_args) {
      Ok(s) => self.print(scope, &format!("{s}\n"), 1),
      Err(err) => rethrow(scope, err),
    }
  }

  #[nofast]
  #[reentrant]
  fn time_end(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[string] label: String,
  ) {
    let start = self.timer_map.borrow_mut().remove(&label);
    let Some(start) = start else {
      self.print(scope, &format!("Timer '{label}' does not exist\n"), 2);
      return;
    };
    let duration = start.elapsed().as_secs_f64() * 1000.0;
    let display = Self::duration_display(duration);
    self.print(scope, &format!("{label}: {display}ms\n"), 1);
  }

  #[nofast]
  #[reentrant]
  fn clear(&self, scope: &mut v8::PinScope<'_, '_>) {
    self.indent_level.set(0.0);
    self.print(scope, "\u{1b}[1;1H", 1);
    self.print(scope, "\u{1b}[0J", 1);
  }

  #[nofast]
  #[reentrant]
  fn table<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    data: v8::Local<'a, v8::Value>,
    properties: v8::Local<'a, v8::Value>,
  ) {
    let Some(cached) = self.intr(scope) else {
      return;
    };
    let intr = Intrinsics::new(&cached);
    if let Err(err) = self.table_impl(scope, &intr, data, properties) {
      rethrow(scope, err);
    }
  }
}

impl Console {
  fn table_impl<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    intr: &Intrinsics<'_>,
    data: v8::Local<'s, v8::Value>,
    properties: v8::Local<'s, v8::Value>,
  ) -> R<()> {
    // Validate `properties`.
    let properties: Option<Vec<v8::Local<'s, v8::Value>>> = if properties
      .is_undefined()
    {
      None
    } else if properties.is_array() {
      let arr = v8::Local::<v8::Array>::try_from(properties).unwrap();
      Some(collect_array(scope, arr))
    } else {
      let type_of = properties.type_of(scope).to_rust_string_lossy(scope);
      let msg = v8_str(
        scope,
        &format!(
          "The 'properties' argument must be of type Array: received type {type_of}"
        ),
      );
      let exc = v8::Exception::error(scope, msg);
      return Err(JsErr(v8::Global::new(scope, exc)));
    };

    if data.is_null() || !(data.is_object() || data.is_function()) {
      // this.log(data)
      let mut ctx = self.console_ctx(scope, intr, false);
      let s = inspect_args_impl(scope, intr, &mut ctx, &[data])?;
      self.print(scope, &format!("{s}\n"), 1);
      return Ok(());
    }
    let data_obj = v8::Local::<v8::Object>::try_from(data).unwrap();

    // stringifyValue: inspectValueWithQuotes with depth 1, compact, no break.
    let no_color = self.no_color(scope, false);
    let mut sctx = default_ctx();
    sctx.colors = !no_color;
    if sctx.colors {
      sctx.stylize = StylizeKind::Theme {
        styles: intr.styles_obj(scope),
        colors: intr.colors_obj(scope),
        js_fn: None,
      };
    }
    sctx.depth = Some(1.0);
    sctx.compact = Compact::Bool(true);
    sctx.break_length = f64::INFINITY;

    let is_set_object = data.is_set();
    let is_map_object = data.is_map();
    let iterator_symbol = v8::Symbol::get_iterator(scope);
    let has_iterator_fn = {
      v8::tc_scope!(tc, scope);
      data_obj
        .get(tc, iterator_symbol.into())
        .map(|v| v.is_function())
        .unwrap_or(false)
    };
    let is_iterator_object =
      !is_set_object && !is_map_object && !data.is_array() && has_iterator_fn;
    let values_key = "Values";
    let index_key = if is_set_object || is_map_object || is_iterator_object {
      "(iter idx)"
    } else {
      "(idx)"
    };

    // resultData
    enum ResultData<'s> {
      Object(v8::Local<'s, v8::Object>),
      MapEntries(Vec<(v8::Local<'s, v8::Value>, v8::Local<'s, v8::Value>)>),
      List(Vec<v8::Local<'s, v8::Value>>),
    }

    let result_data: ResultData<'s> = if is_set_object {
      let set = v8::Local::<v8::Set>::try_from(data).unwrap();
      let arr = set.as_array(scope);
      ResultData::List(collect_array(scope, arr))
    } else if is_map_object {
      let map = v8::Local::<v8::Map>::try_from(data).unwrap();
      let arr = map.as_array(scope);
      let mut entries = Vec::new();
      let mut i = 0u32;
      while i + 1 < arr.length() {
        let k = arr
          .get_index(scope, i)
          .unwrap_or_else(|| v8::undefined(scope).into());
        let v = arr
          .get_index(scope, i + 1)
          .unwrap_or_else(|| v8::undefined(scope).into());
        entries.push((k, v));
        i += 2;
      }
      ResultData::MapEntries(entries)
    } else if is_iterator_object {
      // ArrayFrom(data)
      let values = iterate_to_vec(scope, data)?;
      ResultData::List(values)
    } else {
      ResultData::Object(data_obj)
    };

    // keys = ObjectKeys(resultData)
    let keys: Vec<String> = match &result_data {
      ResultData::Object(obj) => {
        v8::tc_scope!(tc, scope);
        match obj.get_property_names(
          tc,
          v8::GetPropertyNamesArgs {
            mode: v8::KeyCollectionMode::OwnOnly,
            property_filter: v8::PropertyFilter::ONLY_ENUMERABLE
              | v8::PropertyFilter::SKIP_SYMBOLS,
            index_filter: v8::IndexFilter::IncludeIndices,
            key_conversion: v8::KeyConversionMode::ConvertToString,
          },
        ) {
          Some(names) => (0..names.length())
            .filter_map(|i| names.get_index(tc, i))
            .map(|v| v.to_rust_string_lossy(tc))
            .collect(),
          None => Vec::new(),
        }
      }
      ResultData::MapEntries(entries) => {
        (0..entries.len()).map(|i| i.to_string()).collect()
      }
      ResultData::List(values) => {
        (0..values.len()).map(|i| i.to_string()).collect()
      }
    };
    let num_rows = keys.len();

    let get_row_value = |scope: &mut v8::PinScope<'s, '_>,
                         idx: usize,
                         key: &str|
     -> Option<RowValue<'s>> {
      match &result_data {
        ResultData::Object(obj) => {
          let key_v8 = v8_str(scope, key);
          let v = {
            v8::tc_scope!(tc, scope);
            obj.get(tc, key_v8.into())
          };
          v.map(RowValue::Plain)
        }
        ResultData::MapEntries(entries) => {
          entries.get(idx).map(|(k, v)| RowValue::MapEntry(*k, *v))
        }
        ResultData::List(values) => {
          values.get(idx).copied().map(RowValue::Plain)
        }
      }
    };

    enum RowValue<'s> {
      Plain(v8::Local<'s, v8::Value>),
      MapEntry(v8::Local<'s, v8::Value>, v8::Local<'s, v8::Value>),
    }

    let properties_strs: Option<Vec<String>> = properties
      .as_ref()
      .map(|p| p.iter().map(|v| v.to_rust_string_lossy(scope)).collect());

    // objectValues: column name -> rows
    let mut object_values: Vec<(String, Vec<Option<String>>)> = Vec::new();
    if let Some(props) = &properties_strs {
      for name in props {
        object_values.push((name.clone(), vec![Some(String::new()); num_rows]));
      }
    }
    let mut index_keys: Vec<Option<String>> = Vec::new();
    let mut values: Vec<Option<String>> = Vec::new();
    let mut has_primitives = false;

    let stringify = |scope: &mut v8::PinScope<'s, '_>,
                     sctx: &mut Ctx<'s>,
                     value: v8::Local<'s, v8::Value>|
     -> R<String> {
      // inspectValueWithQuotes
      if value.is_string() {
        let s = value.to_rust_string_lossy(scope);
        let abbreviate_size =
          sctx.str_abbreviate_size.unwrap_or(STR_ABBREVIATE_SIZE);
        let len16 = utf16_length(&s);
        let trunc = if (len16 as f64) > abbreviate_size {
          let max = abbreviate_size.max(0.0) as usize;
          let mut out = String::new();
          let mut count = 0usize;
          for c in s.chars() {
            if count >= max {
              break;
            }
            count += c.len_utf16();
            if count <= max {
              out.push(c);
            }
          }
          format!("{out}...")
        } else {
          s
        };
        let quoted =
          quote::quote_string(&trunc, &sctx.quotes, sctx.escape_sequences);
        sctx.stylize(scope, &quoted, "string")
      } else {
        format_value(scope, intr, sctx, value, 0.0, false)
      }
    };

    for (idx, k) in keys.iter().enumerate() {
      let row = get_row_value(scope, idx, k);
      let (value, map_entry) = match row {
        Some(RowValue::Plain(v)) => (v, None),
        Some(RowValue::MapEntry(mk, mv)) => {
          (v8::undefined(scope).into(), Some((mk, mv)))
        }
        None => (v8::undefined(scope).into(), None),
      };

      if let Some((mk, mv)) = map_entry {
        // Map rows expose Key/Values columns.
        for (name, cell) in [("Key", mk), (values_key, mv)] {
          let column = match object_values.iter_mut().find(|(n, _)| n == name) {
            Some((_, rows)) => rows,
            None => {
              object_values
                .push((name.to_string(), vec![Some(String::new()); num_rows]));
              &mut object_values.last_mut().unwrap().1
            }
          };
          column[idx] = Some(stringify(scope, &mut sctx, cell)?);
        }
        values.push(Some(String::new()));
        index_keys.push(Some(k.clone()));
        continue;
      }

      let primitive =
        value.is_null() || !(value.is_object() || value.is_function());
      if properties_strs.is_none() && primitive {
        has_primitives = true;
        values.push(Some(stringify(scope, &mut sctx, value)?));
      } else {
        let value_obj = v8::Local::<v8::Object>::try_from(value).ok();
        let row_keys: Vec<String> = match &properties_strs {
          Some(p) => p.clone(),
          None => match value_obj {
            Some(obj) => {
              v8::tc_scope!(tc, scope);
              match obj.get_property_names(
                tc,
                v8::GetPropertyNamesArgs {
                  mode: v8::KeyCollectionMode::OwnOnly,
                  property_filter: v8::PropertyFilter::ONLY_ENUMERABLE
                    | v8::PropertyFilter::SKIP_SYMBOLS,
                  index_filter: v8::IndexFilter::IncludeIndices,
                  key_conversion: v8::KeyConversionMode::ConvertToString,
                },
              ) {
                Some(names) => (0..names.length())
                  .filter_map(|i| names.get_index(tc, i))
                  .map(|v| v.to_rust_string_lossy(tc))
                  .collect(),
                None => Vec::new(),
              }
            }
            None => Vec::new(),
          },
        };
        for rk in &row_keys {
          if !primitive && let Some(obj) = value_obj {
            let has = {
              v8::tc_scope!(tc, scope);
              let key_v8 = v8_str(tc, rk);
              obj.has(tc, key_v8.into()).unwrap_or(false)
            };
            if has {
              let column = match object_values.iter_mut().find(|(n, _)| n == rk)
              {
                Some((_, rows)) => rows,
                None => {
                  object_values
                    .push((rk.clone(), vec![Some(String::new()); num_rows]));
                  &mut object_values.last_mut().unwrap().1
                }
              };
              let key_v8 = v8_str(scope, rk);
              let cell = js_get(scope, obj, key_v8.into())?;
              column[idx] = Some(stringify(scope, &mut sctx, cell)?);
            }
          }
        }
        values.push(Some(String::new()));
      }
      index_keys.push(Some(k.clone()));
    }

    // `objectValues` is a JS object in the original implementation, so its
    // key order follows JS semantics: integer-like keys first in ascending
    // order, then string keys in insertion order.
    type ColEntry = (String, Vec<Option<String>>);
    let mut string_cols: Vec<ColEntry> = Vec::new();
    let mut index_cols: Vec<(u32, ColEntry)> = Vec::new();
    for entry in object_values {
      match quote::is_canonical_index(&entry.0)
        .then(|| entry.0.parse::<u32>().ok())
        .flatten()
      {
        Some(n) => index_cols.push((n, entry)),
        None => string_cols.push(entry),
      }
    }
    index_cols.sort_by_key(|(n, _)| *n);
    let mut object_values: Vec<(String, Vec<Option<String>>)> =
      index_cols.into_iter().map(|(_, e)| e).collect();
    object_values.append(&mut string_cols);

    let header_keys: Vec<String> =
      object_values.iter().map(|(n, _)| n.clone()).collect();
    let body_values: Vec<Vec<Option<String>>> =
      object_values.into_iter().map(|(_, rows)| rows).collect();
    let mut header_props: Vec<String> = match &properties_strs {
      Some(p) => p.clone(),
      None => {
        let mut hp = header_keys.clone();
        if !is_map_object && has_primitives {
          hp.push(values_key.to_string());
        }
        hp
      }
    };
    let mut header: Vec<String> = vec![index_key.to_string()];
    header.append(&mut header_props);
    header.retain(|h| !h.is_empty());

    let mut body: Vec<Vec<Option<String>>> = vec![index_keys];
    body.extend(body_values);
    body.push(values);

    let table_str = table::cli_table(&header, &body);
    // toTable routes through log.
    let mut ctx = self.console_ctx(scope, intr, false);
    let table_v8 = v8_str(scope, &table_str);
    let s = inspect_args_impl(scope, intr, &mut ctx, &[table_v8.into()])?;
    self.print(scope, &format!("{s}\n"), 1);
    Ok(())
  }
}

/// `ArrayFrom(iterable)` for the console.table iterator case.
fn iterate_to_vec<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
) -> R<Vec<v8::Local<'s, v8::Value>>> {
  let obj = v8::Local::<v8::Object>::try_from(value)
    .map_err(|_| simple_error(scope, "not iterable"))?;
  let iterator_symbol = v8::Symbol::get_iterator(scope);
  let iter_fn = js_get(scope, obj, iterator_symbol.into())?;
  let iter_fn = v8::Local::<v8::Function>::try_from(iter_fn)
    .map_err(|_| simple_error(scope, "not iterable"))?;
  let iterator = js_call(scope, iter_fn, value, &[])?;
  let iterator_obj = v8::Local::<v8::Object>::try_from(iterator)
    .map_err(|_| simple_error(scope, "iterator is not an object"))?;
  let next_fn = js_get_str(scope, iterator_obj, "next")?;
  let next_fn = v8::Local::<v8::Function>::try_from(next_fn)
    .map_err(|_| simple_error(scope, "iterator.next is not a function"))?;
  let mut out = Vec::new();
  loop {
    let step = js_call(scope, next_fn, iterator, &[])?;
    let step_obj = v8::Local::<v8::Object>::try_from(step)
      .map_err(|_| simple_error(scope, "iterator result is not an object"))?;
    let done = js_get_str(scope, step_obj, "done")?;
    if done.boolean_value(scope) {
      break;
    }
    let item = js_get_str(scope, step_obj, "value")?;
    out.push(item);
    if out.len() > 10_000_000 {
      break;
    }
  }
  Ok(out)
}

fn simple_error<'s>(scope: &mut v8::PinScope<'s, '_>, msg: &str) -> JsErr {
  let msg = v8_str(scope, msg);
  let exc = v8::Exception::type_error(scope, msg);
  JsErr(v8::Global::new(scope, exc))
}
