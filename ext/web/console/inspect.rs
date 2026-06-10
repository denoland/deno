// Copyright 2018-2026 the Deno authors. MIT license.

//! Value inspection engine ported from `01_console.js` (`formatValue`,
//! `formatRaw` and friends). The goal is byte-for-byte identical output with
//! the JavaScript implementation it replaces.

use std::collections::HashMap;

use deno_core::v8;

use super::quote;
use super::width::get_string_width;

/// A caught JavaScript exception, propagated like JS exceptions are: most
/// callers re-raise, `format_raw` converts to `[Internal Formatting Error]`.
pub struct JsErr(pub v8::Global<v8::Value>);

pub type R<T> = Result<T, JsErr>;

pub const K_OBJECT_TYPE: u8 = 0;
pub const K_ARRAY_TYPE: u8 = 1;
pub const K_ARRAY_EXTRAS_TYPE: u8 = 2;

const K_MIN_LINE_LENGTH: usize = 16;

// Iterator-state constants from 01_console.js.
const K_WEAK: u8 = 0;
const K_ITERATOR: u8 = 1;
const K_MAP_ENTRIES: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Compact {
  Bool(bool),
  Num(f64),
}

impl Compact {
  pub fn is_true(self) -> bool {
    matches!(self, Compact::Bool(true))
  }
  pub fn as_number(self) -> Option<f64> {
    match self {
      Compact::Num(n) => Some(n),
      Compact::Bool(_) => None,
    }
  }
}

#[derive(Clone, Copy)]
pub enum Sorted<'s> {
  No,
  Yes,
  Comparator(v8::Local<'s, v8::Function>),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Getters {
  No,
  Yes,
  Get,
  Set,
}

#[derive(Clone, Copy)]
pub enum StylizeKind<'s> {
  NoColor,
  /// Style/color lookup tables (the `styles`/`colors` objects); honoring
  /// runtime mutation of e.g. `util.inspect.styles`.
  Theme {
    styles: v8::Local<'s, v8::Object>,
    colors: v8::Local<'s, v8::Object>,
    /// The original JS function, when one exists, so it can be handed to
    /// user custom-inspect hooks unchanged.
    js_fn: Option<v8::Local<'s, v8::Function>>,
  },
  Js(v8::Local<'s, v8::Function>),
}

/// Intrinsic JS values the engine needs, captured by the JS shim at module
/// initialization (so they are primordial-safe) and passed per call. Fields
/// are opened from the cached Globals lazily — most calls (primitives) need
/// none of them.
pub struct Intrinsics<'c> {
  cached: &'c CachedIntrinsics,
}

macro_rules! intr_fn {
  ($name:ident) => {
    pub fn $name<'s>(
      &self,
      scope: &mut v8::PinScope<'s, '_>,
    ) -> v8::Local<'s, v8::Function> {
      v8::Local::new(scope, &self.cached.$name)
    }
  };
}
macro_rules! intr_obj {
  ($name:ident) => {
    pub fn $name<'s>(
      &self,
      scope: &mut v8::PinScope<'s, '_>,
    ) -> v8::Local<'s, v8::Object> {
      v8::Local::new(scope, &self.cached.$name)
    }
  };
}

impl<'c> Intrinsics<'c> {
  pub fn new(cached: &'c CachedIntrinsics) -> Self {
    Intrinsics { cached }
  }

  intr_fn!(function_to_string);
  intr_fn!(inspect_fn);
  intr_fn!(stylize_no_color);
  intr_fn!(create_stylize_with_color);
  intr_fn!(get_url_prototype);
  intr_fn!(get_cwd);
  intr_fn!(make_cross_context_stylize);
  intr_fn!(reg_exp_to_string);
  intr_fn!(number_value_of);
  intr_fn!(string_value_of);
  intr_fn!(boolean_value_of);
  intr_fn!(big_int_value_of);
  intr_fn!(symbol_value_of);
  intr_obj!(styles_obj);
  intr_obj!(colors_obj);
  intr_obj!(object_prototype);
  intr_obj!(error_prototype);

  /// `[prototype, name, constructor]` triples (`wellKnownPrototypes`).
  pub fn well_known<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> Vec<(
    v8::Local<'s, v8::Object>,
    &'c str,
    v8::Local<'s, v8::Function>,
  )> {
    self
      .cached
      .well_known
      .iter()
      .map(|(proto, name, ctor)| {
        (
          v8::Local::new(scope, proto),
          name.as_str(),
          v8::Local::new(scope, ctor),
        )
      })
      .collect()
  }
}

/// Pre-parsed intrinsics as v8 Globals, cached across calls (the intrinsics
/// object is created once by the JS shim and never changes).
pub struct CachedIntrinsics {
  pub key: v8::Global<v8::Object>,
  pub function_to_string: v8::Global<v8::Function>,
  pub inspect_fn: v8::Global<v8::Function>,
  pub stylize_no_color: v8::Global<v8::Function>,
  pub create_stylize_with_color: v8::Global<v8::Function>,
  pub styles_obj: v8::Global<v8::Object>,
  pub colors_obj: v8::Global<v8::Object>,
  pub object_prototype: v8::Global<v8::Object>,
  pub error_prototype: v8::Global<v8::Object>,
  pub well_known:
    Vec<(v8::Global<v8::Object>, String, v8::Global<v8::Function>)>,
  pub get_url_prototype: v8::Global<v8::Function>,
  pub get_cwd: v8::Global<v8::Function>,
  pub make_cross_context_stylize: v8::Global<v8::Function>,
  pub reg_exp_to_string: v8::Global<v8::Function>,
  pub number_value_of: v8::Global<v8::Function>,
  pub string_value_of: v8::Global<v8::Function>,
  pub boolean_value_of: v8::Global<v8::Function>,
  pub big_int_value_of: v8::Global<v8::Function>,
  pub symbol_value_of: v8::Global<v8::Function>,
}

impl CachedIntrinsics {
  pub fn matches<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    obj: v8::Local<'s, v8::Object>,
  ) -> bool {
    let key = v8::Local::new(scope, &self.key);
    let key_val: v8::Local<v8::Value> = key.into();
    key_val.strict_equals(obj.into())
  }
}

pub struct Ctx<'s> {
  pub show_hidden: bool,
  /// `None` == JS `null` (unlimited).
  pub depth: Option<f64>,
  pub colors: bool,
  pub custom_inspect: bool,
  pub show_proxy: bool,
  pub max_array_length: f64,
  pub max_string_length: f64,
  pub break_length: f64,
  pub escape_sequences: bool,
  pub compact: Compact,
  pub sorted: Sorted<'s>,
  pub getters: Getters,
  pub quotes: Vec<String>,
  pub iterable_limit: f64,
  pub trailing_comma: bool,
  pub indent_level: f64,
  pub str_abbreviate_size: Option<f64>,

  // Mutable state.
  pub indentation_lvl: usize,
  pub current_depth: f64,
  pub seen: Vec<v8::Local<'s, v8::Value>>,
  /// Insertion-ordered circular-reference map (`ctx.circular`).
  pub circular: Vec<(v8::Local<'s, v8::Value>, usize)>,
  pub circular_set: bool,
  pub budget: HashMap<usize, usize>,
  pub stylize: StylizeKind<'s>,

  /// `ctx.userOptions` (node compat).
  pub user_options: Option<v8::Local<'s, v8::Object>>,
  /// `ctx.numericSeparator` passthrough for `getUserOptions`.
  pub numeric_separator: Option<v8::Local<'s, v8::Value>>,
  /// `ctx.inspect` — used to filter out util.inspect itself when probing
  /// the node custom-inspect symbol.
  pub ctx_inspect_fn: Option<v8::Local<'s, v8::Value>>,

  /// Memoized `getURLPrototype()` result.
  pub url_prototype: Option<Option<v8::Local<'s, v8::Object>>>,
  /// Memoized canonical circular-JSON error message (`%j` handling).
  pub circular_error_message: Option<String>,
  /// Memoized JS-function form of `stylize` for user hooks.
  pub stylize_js_fn: Option<v8::Local<'s, v8::Function>>,
  /// Per-call memo of resolved theme escape codes, keyed by flavour.
  pub theme_memo: HashMap<&'static str, Option<(String, String)>>,
}

impl<'s> Ctx<'s> {
  pub fn stylize(
    &mut self,
    scope: &mut v8::PinScope<'s, '_>,
    s: &str,
    flavour: &'static str,
  ) -> R<String> {
    if let StylizeKind::Theme { styles, colors, .. } = self.stylize {
      // The styles/colors tables are stable for the duration of a single
      // inspect call; memoize the resolved escape codes per flavour.
      if let Some(memo) = self.theme_memo.get(flavour) {
        return Ok(match memo {
          Some((open, close)) => format!("{open}{s}{close}"),
          None => s.to_string(),
        });
      }
      let resolved = resolve_theme_codes(scope, styles, colors, flavour);
      let result = match &resolved {
        Some((open, close)) => format!("{open}{s}{close}"),
        None => s.to_string(),
      };
      self.theme_memo.insert(flavour, resolved);
      return Ok(result);
    }
    stylize_with(scope, &self.stylize, s, flavour)
  }
}

/// Resolve `styles[flavour]` -> `colors[style]` -> escape codes.
fn resolve_theme_codes<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  styles: v8::Local<'s, v8::Object>,
  colors: v8::Local<'s, v8::Object>,
  flavour: &'static str,
) -> Option<(String, String)> {
  let flavour_key = v8_static_key(scope, flavour);
  let style = styles.get(scope, flavour_key.into())?;
  if style.is_undefined() {
    return None;
  }
  let style_key = style.to_string(scope)?;
  let color = colors.get(scope, style_key.into())?;
  if color.is_undefined() {
    return None;
  }
  let arr = v8::Local::<v8::Object>::try_from(color).ok()?;
  let open = arr.get_index(scope, 0)?.number_value(scope)?;
  let close = arr.get_index(scope, 1)?.number_value(scope)?;
  Some((
    format!("\u{1b}[{}m", fmt_js_number(open)),
    format!("\u{1b}[{}m", fmt_js_number(close)),
  ))
}

pub fn stylize_with<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  kind: &StylizeKind<'s>,
  s: &str,
  flavour: &str,
) -> R<String> {
  match kind {
    StylizeKind::NoColor => Ok(s.to_string()),
    StylizeKind::Theme { styles, colors, .. } => {
      // const style = styles[styleType];
      let flavour_key = v8_str(scope, flavour);
      let style = styles.get(scope, flavour_key.into());
      let Some(style) = style else {
        return Ok(s.to_string());
      };
      if !style.is_string() {
        if style.is_undefined() {
          return Ok(s.to_string());
        }
      }
      let style_key = match style.to_string(scope) {
        Some(v) => v,
        None => return Ok(s.to_string()),
      };
      if style.is_undefined() {
        return Ok(s.to_string());
      }
      let color = colors.get(scope, style_key.into());
      let Some(color) = color else {
        return Ok(s.to_string());
      };
      if color.is_undefined() {
        return Ok(s.to_string());
      }
      let Ok(arr) = v8::Local::<v8::Object>::try_from(color) else {
        return Ok(s.to_string());
      };
      let open = get_index(scope, arr, 0)
        .and_then(|v| v.number_value(scope))
        .unwrap_or(0.0);
      let close = get_index(scope, arr, 1)
        .and_then(|v| v.number_value(scope))
        .unwrap_or(0.0);
      Ok(format!(
        "\u{1b}[{}m{}\u{1b}[{}m",
        fmt_js_number(open),
        s,
        fmt_js_number(close)
      ))
    }
    StylizeKind::Js(f) => {
      let str_val = v8_str(scope, s);
      let flavour_val = v8_str(scope, flavour);
      let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
      let ret =
        js_call(scope, *f, undef, &[str_val.into(), flavour_val.into()])?;
      Ok(to_rust_string(scope, ret))
    }
  }
}

fn fmt_js_number(n: f64) -> String {
  if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e21 {
    format!("{}", n as i64)
  } else {
    format!("{}", n)
  }
}

// ---------------------------------------------------------------------------
// small helpers

pub fn v8_str<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  s: &str,
) -> v8::Local<'s, v8::String> {
  v8::String::new(scope, s).unwrap()
}

/// Internalized string for a static property key: property lookups with an
/// internalized key hit the fast path (no per-access string-table migration
/// like external strings incur).
pub fn v8_static_key<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  s: &'static str,
) -> v8::Local<'s, v8::String> {
  v8::String::new_from_one_byte(
    scope,
    s.as_bytes(),
    v8::NewStringType::Internalized,
  )
  .unwrap_or_else(|| v8_str(scope, s))
}

pub fn to_rust_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> String {
  value.to_rust_string_lossy(scope)
}

fn get_index<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  i: u32,
) -> Option<v8::Local<'s, v8::Value>> {
  obj.get_index(scope, i)
}

pub fn grab_err<'s>(
  tc: &mut v8::PinScope<'s, '_>,
  exception: Option<v8::Local<'s, v8::Value>>,
) -> JsErr {
  let exc = exception.unwrap_or_else(|| v8::undefined(tc).into());
  JsErr(v8::Global::new(tc, exc))
}

/// Call a JS function, catching exceptions.
pub fn js_call<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  f: v8::Local<'s, v8::Function>,
  this: v8::Local<'s, v8::Value>,
  args: &[v8::Local<'s, v8::Value>],
) -> R<v8::Local<'s, v8::Value>> {
  v8::tc_scope!(tc, scope);
  match f.call(tc, this, args) {
    Some(v) => Ok(v),
    None => {
      let exc = tc.exception();
      Err(grab_err(tc, exc))
    }
  }
}

/// `obj[key]` with exceptions propagated as `Err`.
pub fn js_get<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  obj: v8::Local<'s, v8::Object>,
  key: v8::Local<'s, v8::Value>,
) -> R<v8::Local<'s, v8::Value>> {
  v8::tc_scope!(tc, scope);
  match obj.get(tc, key) {
    Some(v) => Ok(v),
    None => {
      let exc = tc.exception();
      Err(grab_err(tc, exc))
    }
  }
}

pub fn js_get_str<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  obj: v8::Local<'s, v8::Object>,
  key: &'static str,
) -> R<v8::Local<'s, v8::Value>> {
  let key = v8_static_key(scope, key);
  js_get(scope, obj, key.into())
}

/// `obj[key]`, swallowing exceptions (mirrors JS `try { ... } catch {}`).
pub fn try_get<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  obj: v8::Local<'s, v8::Object>,
  key: v8::Local<'s, v8::Value>,
) -> Option<v8::Local<'s, v8::Value>> {
  v8::tc_scope!(tc, scope);
  obj.get(tc, key)
}

pub fn try_get_str<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  obj: v8::Local<'s, v8::Object>,
  key: &'static str,
) -> Option<v8::Local<'s, v8::Value>> {
  let key = v8_static_key(scope, key);
  try_get(scope, obj, key.into())
}

/// `ObjectPrototypeIsPrototypeOf(proto, value)` — walk the prototype chain.
pub fn is_prototype_of<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  proto: v8::Local<'s, v8::Value>,
  value: v8::Local<'s, v8::Value>,
) -> bool {
  let Ok(mut obj) = v8::Local::<v8::Object>::try_from(value) else {
    return false;
  };
  loop {
    let Some(p) = obj.get_prototype(scope) else {
      return false;
    };
    if p.is_null() {
      return false;
    }
    if p.strict_equals(proto) {
      return true;
    }
    let Ok(p_obj) = v8::Local::<v8::Object>::try_from(p) else {
      return false;
    };
    obj = p_obj;
  }
}

fn includes_identity<'s>(
  list: &[v8::Local<'s, v8::Value>],
  value: v8::Local<'s, v8::Value>,
) -> bool {
  list.iter().any(|v| v.strict_equals(value))
}

/// `isUndetectableObject`: typeof is "undefined" but value is not undefined
/// (document.all style). v8: value.is_undefined() false but TypeOf says
/// undefined.
fn is_undetectable<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> bool {
  if value.is_undefined() {
    return false;
  }
  let type_of = value.type_of(scope).to_rust_string_lossy(scope);
  type_of == "undefined"
}

pub fn symbol_for<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  description: &str,
) -> v8::Local<'s, v8::Symbol> {
  let key = v8_str(scope, description);
  v8::Symbol::for_key(scope, key)
}

// ---------------------------------------------------------------------------
// formatValue

pub fn format_value<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
  typed_array: bool,
) -> R<String> {
  // Primitive types cannot have properties.
  if !value.is_object()
    && !value.is_function()
    && !is_undetectable(scope, value)
  {
    if value.is_null() {
      return ctx.stylize(scope, "null", "null");
    }
    return format_primitive(scope, ctx, value, false);
  }
  if value.is_null() {
    return ctx.stylize(scope, "null", "null");
  }

  // Memorize the context for custom inspection on proxies.
  let context = value;
  let mut value = value;
  let mut proxy_details: Option<(
    v8::Local<'s, v8::Value>,
    v8::Local<'s, v8::Value>,
  )> = None;
  if let Ok(proxy) = v8::Local::<v8::Proxy>::try_from(value) {
    let target = proxy.get_target(scope);
    let handler = proxy.get_handler(scope);
    if !ctx.show_proxy {
      // Inspect the proxy target directly; avoids invoking proxy traps.
      value = target;
    } else {
      proxy_details = Some((target, handler));
    }
  }

  // Provide a hook for user-specified inspect functions.
  if ctx.custom_inspect {
    if let Some(result) = try_custom_inspect(
      scope,
      intr,
      ctx,
      context,
      value,
      proxy_details,
      recurse_times,
    )? {
      return Ok(result);
    }
  }

  // Circular reference detection.
  if includes_identity(&ctx.seen, value) {
    let mut index = 1;
    if !ctx.circular_set {
      ctx.circular_set = true;
      ctx.circular.push((value, index));
    } else {
      match ctx.circular.iter().find(|(v, _)| v.strict_equals(value)) {
        Some((_, i)) => index = *i,
        None => {
          index = ctx.circular.len() + 1;
          ctx.circular.push((value, index));
        }
      }
    }
    return ctx.stylize(scope, &format!("[Circular *{index}]"), "special");
  }

  format_raw(
    scope,
    intr,
    ctx,
    value,
    recurse_times,
    typed_array,
    proxy_details,
  )
}

/// Returns `Ok(Some(string))` when a custom-inspect hook produced output.
fn try_custom_inspect<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  context: v8::Local<'s, v8::Value>,
  value: v8::Local<'s, v8::Value>,
  proxy_details: Option<(v8::Local<'s, v8::Value>, v8::Local<'s, v8::Value>)>,
  recurse_times: f64,
) -> R<Option<String>> {
  let inspect_target = match proxy_details {
    Some((target, _)) => target,
    None => value,
  };
  let Ok(inspect_target_obj) =
    v8::Local::<v8::Object>::try_from(inspect_target)
  else {
    return Ok(None);
  };
  let Ok(value_obj) = v8::Local::<v8::Object>::try_from(value) else {
    return Ok(None);
  };

  let custom_inspect = symbol_for(scope, "Deno.customInspect");
  let private_custom_inspect = symbol_for(scope, "Deno.privateCustomInspect");

  for (sym, _private) in
    [(custom_inspect, false), (private_custom_inspect, true)]
  {
    // ReflectHas walks the prototype chain.
    let has = {
      v8::tc_scope!(tc, scope);
      inspect_target_obj.has(tc, sym.into()).unwrap_or(false)
    };
    if has {
      let func = js_get(scope, inspect_target_obj, sym.into())?;
      if let Ok(func) = v8::Local::<v8::Function>::try_from(func) {
        // return String(value[customInspect](inspect, ctx));
        let ctx_obj = materialize_ctx(scope, intr, ctx)?;
        let inspect_fn = intr.inspect_fn(scope);
        let ret =
          js_call(scope, func, value, &[inspect_fn.into(), ctx_obj.into()])?;
        // Read back state the hook may have changed through nested
        // `inspect(..., ctx)` calls (seen/circular are shared in JS).
        readback_ctx(scope, ctx, ctx_obj);
        let s = {
          v8::tc_scope!(tc, scope);
          match ret.to_string(tc) {
            Some(s) => s.to_rust_string_lossy(tc),
            None => {
              let exc = tc.exception();
              return Err(grab_err(tc, exc));
            }
          }
        };
        return Ok(Some(s));
      }
    }
  }

  // node custom inspect symbol.
  let node_sym = symbol_for(scope, "nodejs.util.inspect.custom");
  let maybe_custom = {
    v8::tc_scope!(tc, scope);
    inspect_target_obj.get(tc, node_sym.into())
  };
  let Some(maybe_custom) = maybe_custom else {
    return Ok(None);
  };
  let Ok(custom_fn) = v8::Local::<v8::Function>::try_from(maybe_custom) else {
    return Ok(None);
  };

  // Filter out the util module's own inspect function.
  if let Some(ctx_inspect) = ctx.ctx_inspect_fn {
    if maybe_custom.strict_equals(ctx_inspect) {
      return Ok(None);
    }
  }
  // Filter out prototype objects: value.constructor &&
  // value.constructor.prototype === value
  {
    let ctor = try_get_str(scope, value_obj, "constructor");
    if let Some(ctor) = ctor {
      if let Ok(ctor_obj) = v8::Local::<v8::Object>::try_from(ctor) {
        if ctor.is_object() || ctor.is_function() {
          if let Some(proto) = try_get_str(scope, ctor_obj, "prototype") {
            if proto.strict_equals(value) {
              return Ok(None);
            }
          }
        }
      }
    }
  }

  // depth = ctx.depth === null ? null : ctx.depth - recurseTimes
  let depth_val: v8::Local<v8::Value> = match ctx.depth {
    None => v8::null(scope).into(),
    Some(d) => v8::Number::new(scope, d - recurse_times).into(),
  };
  let object_prototype = intr.object_prototype(scope);
  let is_cross_context =
    !is_prototype_of(scope, object_prototype.into(), context);
  let user_options = get_user_options(scope, intr, ctx, is_cross_context)?;
  let ctx_inspect: v8::Local<v8::Value> = match ctx.ctx_inspect_fn {
    Some(f) => f,
    None => intr.inspect_fn(scope).into(),
  };
  let ret = js_call(
    scope,
    custom_fn,
    context,
    &[depth_val, user_options.into(), ctx_inspect],
  )?;

  // If the custom inspection method returned `this`, don't go into infinite
  // recursion.
  if ret.strict_equals(context) {
    return Ok(None);
  }
  if !ret.is_string() {
    return format_value(scope, intr, ctx, ret, recurse_times, false).map(Some);
  }
  let s = to_rust_string(scope, ret);
  let indent = " ".repeat(ctx.indentation_lvl);
  Ok(Some(s.replace('\n', &format!("\n{indent}"))))
}

/// `getUserOptions(ctx, isCrossContext)`.
fn get_user_options<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  is_cross_context: bool,
) -> R<v8::Local<'s, v8::Object>> {
  let ret = v8::Object::new(scope);
  let stylize_fn = ctx_stylize_js_fn(scope, intr, ctx)?;
  set_prop(scope, ret, "stylize", stylize_fn.into());
  set_bool(scope, ret, "showHidden", ctx.show_hidden);
  let depth: v8::Local<v8::Value> = match ctx.depth {
    None => v8::null(scope).into(),
    Some(d) => v8::Number::new(scope, d).into(),
  };
  set_prop(scope, ret, "depth", depth);
  set_bool(scope, ret, "colors", ctx.colors);
  set_bool(scope, ret, "customInspect", ctx.custom_inspect);
  set_bool(scope, ret, "showProxy", ctx.show_proxy);
  set_num(scope, ret, "maxArrayLength", ctx.max_array_length);
  set_num(scope, ret, "maxStringLength", ctx.max_string_length);
  set_num(scope, ret, "breakLength", ctx.break_length);
  let compact: v8::Local<v8::Value> = match ctx.compact {
    Compact::Bool(b) => v8::Boolean::new(scope, b).into(),
    Compact::Num(n) => v8::Number::new(scope, n).into(),
  };
  set_prop(scope, ret, "compact", compact);
  let sorted: v8::Local<v8::Value> = match ctx.sorted {
    Sorted::No => v8::Boolean::new(scope, false).into(),
    Sorted::Yes => v8::Boolean::new(scope, true).into(),
    Sorted::Comparator(f) => f.into(),
  };
  set_prop(scope, ret, "sorted", sorted);
  let getters: v8::Local<v8::Value> = match ctx.getters {
    Getters::No => v8::Boolean::new(scope, false).into(),
    Getters::Yes => v8::Boolean::new(scope, true).into(),
    Getters::Get => v8_str(scope, "get").into(),
    Getters::Set => v8_str(scope, "set").into(),
  };
  set_prop(scope, ret, "getters", getters);
  let numeric_separator: v8::Local<v8::Value> = match ctx.numeric_separator {
    Some(v) => v,
    None => v8::undefined(scope).into(),
  };
  set_prop(scope, ret, "numericSeparator", numeric_separator);

  // ...ctx.userOptions
  if let Some(user_options) = ctx.user_options {
    copy_enumerable_own_props(scope, user_options, ret)?;
  }

  if is_cross_context {
    // Remove the prototype and all non-primitive values; wrap stylize.
    let null_val: v8::Local<v8::Value> = v8::null(scope).into();
    ret.set_prototype(scope, null_val);
    let names = {
      v8::tc_scope!(tc, scope);
      ret.get_own_property_names(tc, Default::default())
    };
    if let Some(names) = names {
      for i in 0..names.length() {
        let Some(key) = names.get_index(scope, i) else {
          continue;
        };
        let Some(val) = try_get(scope, ret, key) else {
          continue;
        };
        if (val.is_object() || val.is_function()) && !val.is_null() {
          v8::tc_scope!(tc, scope);
          ret.delete(tc, key);
        }
      }
    }
    let make_cross_context_stylize = intr.make_cross_context_stylize(scope);
    let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
    let wrapped = js_call(
      scope,
      make_cross_context_stylize,
      undef,
      &[stylize_fn.into()],
    )?;
    set_prop(scope, ret, "stylize", wrapped);
  }

  Ok(ret)
}

fn copy_enumerable_own_props<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  from: v8::Local<'s, v8::Object>,
  to: v8::Local<'s, v8::Object>,
) -> R<()> {
  let names = {
    v8::tc_scope!(tc, scope);
    from.get_own_property_names(
      tc,
      v8::GetPropertyNamesArgs {
        mode: v8::KeyCollectionMode::OwnOnly,
        property_filter: v8::PropertyFilter::ONLY_ENUMERABLE,
        index_filter: v8::IndexFilter::IncludeIndices,
        ..Default::default()
      },
    )
  };
  if let Some(names) = names {
    for i in 0..names.length() {
      let Some(key) = names.get_index(scope, i) else {
        continue;
      };
      let val = js_get(scope, from, key)?;
      set_prop_key(scope, to, key, val);
    }
  }
  Ok(())
}

pub fn set_prop<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: &str,
  value: v8::Local<'s, v8::Value>,
) {
  let key = v8_str(scope, key);
  obj.set(scope, key.into(), value);
}

fn set_prop_key<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: v8::Local<'s, v8::Value>,
  value: v8::Local<'s, v8::Value>,
) {
  obj.set(scope, key, value);
}

fn set_bool<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: &str,
  value: bool,
) {
  let v = v8::Boolean::new(scope, value);
  set_prop(scope, obj, key, v.into());
}

fn set_num<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: &str,
  value: f64,
) {
  let v = v8::Number::new(scope, value);
  set_prop(scope, obj, key, v.into());
}

/// The JS-function form of `ctx.stylize`, for handing to user hooks.
pub fn ctx_stylize_js_fn<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
) -> R<v8::Local<'s, v8::Function>> {
  if let Some(f) = ctx.stylize_js_fn {
    return Ok(f);
  }
  let f = match ctx.stylize {
    StylizeKind::Js(f) => f,
    StylizeKind::Theme { js_fn: Some(f), .. } => f,
    StylizeKind::Theme {
      styles,
      colors,
      js_fn: None,
    } => {
      let create_stylize_with_color = intr.create_stylize_with_color(scope);
      let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
      let ret = js_call(
        scope,
        create_stylize_with_color,
        undef,
        &[styles.into(), colors.into()],
      )?;
      v8::Local::<v8::Function>::try_from(ret)
        .unwrap_or_else(|_| intr.stylize_no_color(scope))
    }
    StylizeKind::NoColor => intr.stylize_no_color(scope),
  };
  ctx.stylize_js_fn = Some(f);
  Ok(f)
}

/// Materialize the engine context as a JS ctx object compatible with the
/// historical `inspectOptions`/ctx shape, for `Deno.customInspect` hooks
/// that pass it back into `inspect(value, ctx)`.
pub fn materialize_ctx<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
) -> R<v8::Local<'s, v8::Object>> {
  let obj = get_user_options(scope, intr, ctx, false)?;
  set_num(scope, obj, "indentationLvl", ctx.indentation_lvl as f64);
  set_num(scope, obj, "currentDepth", ctx.current_depth);
  set_num(scope, obj, "indentLevel", ctx.indent_level);
  set_bool(scope, obj, "escapeSequences", ctx.escape_sequences);
  set_bool(scope, obj, "trailingComma", ctx.trailing_comma);
  set_num(scope, obj, "iterableLimit", ctx.iterable_limit);
  if let Some(n) = ctx.str_abbreviate_size {
    set_num(scope, obj, "strAbbreviateSize", n);
  }
  // quotes
  let quote_vals: Vec<v8::Local<v8::Value>> =
    ctx.quotes.iter().map(|q| v8_str(scope, q).into()).collect();
  let quotes = v8::Array::new_with_elements(scope, &quote_vals);
  set_prop(scope, obj, "quotes", quotes.into());
  // seen (shared identity matters for circular detection in nested calls)
  let seen_vals: Vec<v8::Local<v8::Value>> = ctx.seen.clone();
  let seen = v8::Array::new_with_elements(scope, &seen_vals);
  set_prop(scope, obj, "seen", seen.into());
  // budget
  let budget = v8::Object::new(scope);
  for (k, v) in &ctx.budget {
    let key = v8_str(scope, &k.to_string());
    let val = v8::Number::new(scope, *v as f64);
    budget.set(scope, key.into(), val.into());
  }
  set_prop(scope, obj, "budget", budget.into());
  let inspect_fn = intr.inspect_fn(scope);
  set_prop(scope, obj, "inspect", inspect_fn.into());
  Ok(obj)
}

/// Read back `seen` mutations after a custom-inspect hook ran (nested
/// inspect calls share the array in the JS implementation; we re-sync).
fn readback_ctx<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  ctx: &mut Ctx<'s>,
  ctx_obj: v8::Local<'s, v8::Object>,
) {
  if let Some(seen) = try_get_str(scope, ctx_obj, "seen") {
    if let Ok(arr) = v8::Local::<v8::Array>::try_from(seen) {
      let mut new_seen = Vec::with_capacity(arr.length() as usize);
      for i in 0..arr.length() {
        if let Some(v) = arr.get_index(scope, i) {
          new_seen.push(v);
        }
      }
      ctx.seen = new_seen;
    }
  }
}

// ---------------------------------------------------------------------------
// primitives

fn number_display<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> String {
  let n = value.number_value(scope).unwrap_or(f64::NAN);
  if n == 0.0 && n.is_sign_negative() {
    "-0".to_string()
  } else {
    let num = v8::Number::new(scope, n);
    num.to_rust_string_lossy(scope)
  }
}

pub fn format_number<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
) -> R<String> {
  let s = number_display(scope, value);
  ctx.stylize(scope, &s, "number")
}

pub fn format_bigint<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
) -> R<String> {
  let s = value.to_rust_string_lossy(scope);
  ctx.stylize(scope, &format!("{s}n"), "bigint")
}

/// `maybeQuoteSymbol`.
fn maybe_quote_symbol<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  ctx: &Ctx<'s>,
  symbol: v8::Local<'s, v8::Symbol>,
) -> String {
  let desc = symbol.description(scope);
  if desc.is_undefined() {
    return symbol_to_string(scope, symbol);
  }
  let desc_str = desc.to_rust_string_lossy(scope);
  if quote::symbol_description_needs_no_quotes(&desc_str) {
    return symbol_to_string(scope, symbol);
  }
  format!(
    "Symbol({})",
    quote::quote_string(&desc_str, &ctx.quotes, ctx.escape_sequences)
  )
}

fn symbol_to_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  symbol: v8::Local<'s, v8::Symbol>,
) -> String {
  let desc = symbol.description(scope);
  if desc.is_undefined() {
    "Symbol()".to_string()
  } else {
    format!("Symbol({})", desc.to_rust_string_lossy(scope))
  }
}

/// `formatPrimitive(fn, value, ctx)`. `no_color` is used by `getBoxedBase`,
/// which always formats the inner primitive without color.
pub fn format_primitive<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  no_color: bool,
) -> R<String> {
  macro_rules! style {
    ($scope:expr, $text:expr, $flavour:expr) => {
      if no_color {
        Ok::<String, JsErr>($text.to_string())
      } else {
        ctx.stylize($scope, &$text, $flavour)
      }
    };
  }
  if value.is_string() {
    let mut s = value.to_rust_string_lossy(scope);
    let mut trailer = String::new();
    let utf16_len = v8::Local::<v8::String>::try_from(value)
      .map(|s| s.length())
      .unwrap_or(s.chars().count());
    if (utf16_len as f64) > ctx.max_string_length {
      let max = ctx.max_string_length as usize;
      let remaining = utf16_len - max;
      s = slice_utf16(&s, 0, max);
      trailer = format!(
        "... {} more character{}",
        remaining,
        if remaining > 1 { "s" } else { "" }
      );
    }
    let s_utf16_len = utf16_length(&s);
    if !ctx.compact.is_true()
      && s_utf16_len > K_MIN_LINE_LENGTH
      && (s_utf16_len as f64)
        > ctx.break_length - ctx.indentation_lvl as f64 - 4.0
    {
      // Split on lookahead-after-\n; each piece keeps its trailing \n.
      let mut parts: Vec<String> = Vec::new();
      let mut cur = String::new();
      for c in s.chars() {
        cur.push(c);
        if c == '\n' {
          parts.push(std::mem::take(&mut cur));
        }
      }
      if !cur.is_empty() || parts.is_empty() {
        parts.push(cur);
      }
      let mut out_parts = Vec::with_capacity(parts.len());
      for line in &parts {
        let quoted =
          quote::quote_string(line, &ctx.quotes, ctx.escape_sequences);
        out_parts.push(style!(scope, quoted, "string")?);
      }
      let joiner = format!(" +\n{}", " ".repeat(ctx.indentation_lvl + 2));
      return Ok(out_parts.join(&joiner) + &trailer);
    }
    let quoted = quote::quote_string(&s, &ctx.quotes, ctx.escape_sequences);
    return Ok(style!(scope, quoted, "string")? + &trailer);
  }
  if value.is_number() {
    let s = number_display(scope, value);
    return style!(scope, s, "number");
  }
  if value.is_big_int() {
    let s = format!("{}n", value.to_rust_string_lossy(scope));
    return style!(scope, s, "bigint");
  }
  if value.is_boolean() {
    let s = if value.is_true() { "true" } else { "false" };
    return style!(scope, s, "boolean");
  }
  if value.is_undefined() {
    return style!(scope, "undefined", "undefined");
  }
  // es6 symbol primitive
  let sym = v8::Local::<v8::Symbol>::try_from(value).unwrap();
  let s = maybe_quote_symbol(scope, ctx, sym);
  style!(scope, s, "symbol")
}

/// Slice a Rust string by UTF-16 code-unit indices (JS string semantics).
fn slice_utf16(s: &str, start: usize, end: usize) -> String {
  let mut out = String::new();
  let mut idx = 0usize;
  for c in s.chars() {
    let len = c.len_utf16();
    if idx >= end {
      break;
    }
    if idx >= start {
      // A surrogate pair straddling the boundary gets dropped whole; JS
      // would produce a lone surrogate, which can't be represented in a
      // Rust string. This only affects truncation mid-pair.
      if idx + len <= end {
        out.push(c);
      }
    }
    idx += len;
  }
  out
}

pub fn utf16_length(s: &str) -> usize {
  s.chars().map(|c| c.len_utf16()).sum()
}

// ---------------------------------------------------------------------------
// getPrefix / getCtxStyle

pub fn get_prefix(
  constructor: Option<&str>,
  tag: &str,
  fallback: &str,
  size: &str,
) -> String {
  match constructor {
    None => {
      if !tag.is_empty() && fallback != tag {
        format!("[{fallback}{size}: null prototype] [{tag}] ")
      } else {
        format!("[{fallback}{size}: null prototype] ")
      }
    }
    Some(constructor) => {
      if !tag.is_empty() && constructor != tag {
        format!("{constructor}{size} [{tag}] ")
      } else {
        format!("{constructor}{size} ")
      }
    }
  }
}

fn get_ctx_style<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
  constructor: Option<&str>,
  tag: &str,
) -> String {
  let mut fallback = String::new();
  if constructor.is_none() {
    fallback = v8::Local::<v8::Object>::try_from(value)
      .map(|o| o.get_constructor_name().to_rust_string_lossy(scope))
      .unwrap_or_default();
    if fallback == tag {
      fallback = "Object".to_string();
    }
  }
  get_prefix(constructor, tag, &fallback, "")
}

// ---------------------------------------------------------------------------
// keys

/// `getKeys(value, showHidden)`. Returns property keys (strings + symbols).
pub fn get_keys<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  value: v8::Local<'s, v8::Object>,
  show_hidden: bool,
) -> Vec<v8::Local<'s, v8::Value>> {
  let mut keys: Vec<v8::Local<'s, v8::Value>> = Vec::new();

  let is_module_namespace = v8::Local::<v8::Value>::try_from(value)
    .map(|v| v.is_module_namespace_object())
    .unwrap_or(false);
  let key_collection_mode = if is_module_namespace {
    v8::KeyCollectionMode::IncludePrototypes
  } else {
    v8::KeyCollectionMode::OwnOnly
  };

  // Symbols first (collected separately, appended after names).
  let symbols: Vec<v8::Local<'s, v8::Value>> = {
    v8::tc_scope!(tc, scope);
    match value.get_property_names(
      tc,
      v8::GetPropertyNamesArgs {
        mode: key_collection_mode,
        property_filter: v8::PropertyFilter::SKIP_STRINGS,
        index_filter: v8::IndexFilter::IncludeIndices,
        key_conversion: v8::KeyConversionMode::KeepNumbers,
      },
    ) {
      Some(arr) => {
        let mut out = Vec::with_capacity(arr.length() as usize);
        for i in 0..arr.length() {
          if let Some(v) = arr.get_index(tc, i) {
            out.push(v);
          }
        }
        out
      }
      None => Vec::new(),
    }
  };

  if show_hidden {
    // ObjectGetOwnPropertyNames: all own string keys (incl. non-enumerable).
    v8::tc_scope!(tc, scope);
    if let Some(arr) = value.get_property_names(
      tc,
      v8::GetPropertyNamesArgs {
        mode: key_collection_mode,
        property_filter: v8::PropertyFilter::SKIP_SYMBOLS,
        index_filter: v8::IndexFilter::IncludeIndices,
        key_conversion: v8::KeyConversionMode::ConvertToString,
      },
    ) {
      for i in 0..arr.length() {
        if let Some(v) = arr.get_index(tc, i) {
          keys.push(v);
        }
      }
    }
    keys.extend(symbols.iter().copied());
  } else {
    // ObjectKeys: own enumerable string keys.
    {
      v8::tc_scope!(tc, scope);
      if let Some(arr) = value.get_property_names(
        tc,
        v8::GetPropertyNamesArgs {
          mode: key_collection_mode,
          property_filter: v8::PropertyFilter::ONLY_ENUMERABLE
            | v8::PropertyFilter::SKIP_SYMBOLS,
          index_filter: v8::IndexFilter::IncludeIndices,
          key_conversion: v8::KeyConversionMode::ConvertToString,
        },
      ) {
        for i in 0..arr.length() {
          if let Some(v) = arr.get_index(tc, i) {
            keys.push(v);
          }
        }
      }
    }
    // Filter symbols by enumerability.
    for sym in symbols {
      let enumerable = {
        v8::tc_scope!(tc, scope);
        let desc = v8::Local::<v8::Name>::try_from(sym)
          .ok()
          .and_then(|name| value.get_own_property_descriptor(tc, name));
        match desc {
          Some(desc) if desc.is_object() => {
            let desc_obj = desc.cast::<v8::Object>();
            try_get_str(tc, desc_obj, "enumerable")
              .map(|v| v.is_true())
              .unwrap_or(false)
          }
          _ => false,
        }
      };
      if enumerable {
        keys.push(sym);
      }
    }
  }

  // Errors hide the `cause` property.
  let error_prototype = intr.error_prototype(scope);
  if is_prototype_of(scope, error_prototype.into(), value.into()) {
    keys.retain(|key| {
      if !key.is_string() {
        return true;
      }
      key.to_rust_string_lossy(scope) != "cause"
    });
  }

  keys
}

/// `op_get_non_index_property_names(value, filter)` equivalent: own
/// non-index string property names. `only_enumerable` corresponds to the JS
/// callers' filter value 2 (ONLY_ENUMERABLE); 0 means all.
pub fn get_non_index_property_names<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Object>,
  only_enumerable: bool,
) -> Vec<v8::Local<'s, v8::Value>> {
  let mut property_filter = v8::PropertyFilter::ALL_PROPERTIES;
  if only_enumerable {
    property_filter = property_filter | v8::PropertyFilter::ONLY_ENUMERABLE;
  }
  v8::tc_scope!(tc, scope);
  match value.get_property_names(
    tc,
    v8::GetPropertyNamesArgs {
      mode: v8::KeyCollectionMode::OwnOnly,
      property_filter,
      index_filter: v8::IndexFilter::SkipIndices,
      key_conversion: v8::KeyConversionMode::KeepNumbers,
    },
  ) {
    Some(arr) => {
      let mut out = Vec::with_capacity(arr.length() as usize);
      for i in 0..arr.length() {
        if let Some(v) = arr.get_index(tc, i) {
          out.push(v);
        }
      }
      out
    }
    None => Vec::new(),
  }
}

// ---------------------------------------------------------------------------
// constructor names / prototype properties

/// `builtInObjects`: own property names of the global object matching
/// `^[A-Z][a-zA-Z0-9]+$`. Resolved live like the JS module-level snapshot.
fn built_in_objects<'s>(scope: &mut v8::PinScope<'s, '_>) -> Vec<String> {
  let context = scope.get_current_context();
  let global = context.global(scope);
  let mut out = Vec::new();
  let names = {
    v8::tc_scope!(tc, scope);
    global.get_property_names(
      tc,
      v8::GetPropertyNamesArgs {
        mode: v8::KeyCollectionMode::OwnOnly,
        property_filter: v8::PropertyFilter::SKIP_SYMBOLS,
        index_filter: v8::IndexFilter::SkipIndices,
        key_conversion: v8::KeyConversionMode::ConvertToString,
      },
    )
  };
  if let Some(names) = names {
    for i in 0..names.length() {
      let Some(name) = names.get_index(scope, i) else {
        continue;
      };
      let name = name.to_rust_string_lossy(scope);
      let mut chars = name.chars();
      let first_ok = matches!(chars.next(), Some(c) if c.is_ascii_uppercase());
      if first_ok
        && name.chars().count() >= 2
        && chars.all(|c| c.is_ascii_alphanumeric())
      {
        out.push(name);
      }
    }
  }
  out
}

fn is_instanceof<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  proto: v8::Local<'s, v8::Value>,
  object: v8::Local<'s, v8::Value>,
) -> bool {
  // `isInstanceof` in JS is ObjectPrototypeIsPrototypeOf with a try/catch.
  if !proto.is_object() {
    return false;
  }
  is_prototype_of(scope, proto, object)
}

/// `addPrototypeProperties(ctx, main, obj, recurseTimes, output)`.
#[allow(clippy::too_many_arguments)]
fn add_prototype_properties<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  main: v8::Local<'s, v8::Value>,
  obj_in: v8::Local<'s, v8::Value>,
  recurse_times: f64,
  output: &mut Vec<String>,
) -> R<()> {
  let builtins = built_in_objects(scope);
  let mut depth = 0;
  let mut obj = obj_in;
  let mut keys: Vec<v8::Local<'s, v8::Value>> = Vec::new();
  let mut key_set: Vec<String> = Vec::new();
  loop {
    if depth != 0 || main.strict_equals(obj) {
      let Ok(cur) = v8::Local::<v8::Object>::try_from(obj) else {
        return Ok(());
      };
      let proto = cur.get_prototype(scope);
      let Some(proto) = proto else {
        return Ok(());
      };
      if proto.is_null() {
        return Ok(());
      }
      obj = proto;
      let Ok(obj_o) = v8::Local::<v8::Object>::try_from(obj) else {
        return Ok(());
      };
      // Stop as soon as a built-in object type is detected.
      let descriptor = get_own_descriptor(scope, obj_o, "constructor");
      if let Some(desc) = descriptor {
        if let Some(value) = try_get_str(scope, desc, "value") {
          if value.is_function() {
            if let Ok(f) = v8::Local::<v8::Function>::try_from(value) {
              let name_key = v8_str(scope, "name");
              let name = {
                v8::tc_scope!(tc, scope);
                f.get(tc, name_key.into())
              };
              if let Some(name) = name {
                let name = name.to_rust_string_lossy(scope);
                if builtins.iter().any(|b| *b == name) {
                  return Ok(());
                }
              }
            }
          }
        }
      }
    }

    if depth == 0 {
      key_set.clear();
    } else {
      for key in &keys {
        key_set.push(key.to_rust_string_lossy(scope));
      }
    }
    let Ok(obj_o) = v8::Local::<v8::Object>::try_from(obj) else {
      return Ok(());
    };
    // ReflectOwnKeys(obj)
    keys = {
      v8::tc_scope!(tc, scope);
      match obj_o.get_property_names(
        tc,
        v8::GetPropertyNamesArgs {
          mode: v8::KeyCollectionMode::OwnOnly,
          property_filter: v8::PropertyFilter::ALL_PROPERTIES,
          index_filter: v8::IndexFilter::IncludeIndices,
          key_conversion: v8::KeyConversionMode::ConvertToString,
        },
      ) {
        Some(arr) => {
          let mut out = Vec::with_capacity(arr.length() as usize);
          for i in 0..arr.length() {
            if let Some(v) = arr.get_index(tc, i) {
              out.push(v);
            }
          }
          out
        }
        None => Vec::new(),
      }
    };
    ctx.seen.push(main);
    let main_obj = v8::Local::<v8::Object>::try_from(main).ok();
    for key in keys.clone() {
      // Ignore the `constructor` property and keys that exist on layers
      // above.
      let key_str = if key.is_string() {
        Some(key.to_rust_string_lossy(scope))
      } else {
        None
      };
      if key_str.as_deref() == Some("constructor") {
        continue;
      }
      if let Some(main_obj) = main_obj {
        let has_own = {
          v8::tc_scope!(tc, scope);
          v8::Local::<v8::Name>::try_from(key)
            .ok()
            .and_then(|name| main_obj.has_own_property(tc, name))
            .unwrap_or(false)
        };
        if has_own {
          continue;
        }
      }
      if depth != 0 {
        if let Some(s) = &key_str {
          if key_set.iter().any(|k| k == s) {
            continue;
          }
        }
      }
      let desc = {
        v8::tc_scope!(tc, scope);
        v8::Local::<v8::Name>::try_from(key)
          .ok()
          .and_then(|name| obj_o.get_own_property_descriptor(tc, name))
      };
      let Some(desc) = desc else {
        continue;
      };
      let Ok(desc) = v8::Local::<v8::Object>::try_from(desc) else {
        continue;
      };
      if let Some(value) = try_get_str(scope, desc, "value") {
        if value.is_function() {
          continue;
        }
      }
      let value = format_property(
        scope,
        intr,
        ctx,
        obj,
        recurse_times,
        key,
        K_OBJECT_TYPE,
        Some(desc),
        Some(main),
      )?;
      if ctx.colors {
        output.push(format!("\u{1b}[2m{value}\u{1b}[22m"));
      } else {
        output.push(value);
      }
    }
    ctx.seen.pop();
    depth += 1;
    if depth == 3 {
      break;
    }
  }
  Ok(())
}

fn get_own_descriptor<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  obj: v8::Local<'s, v8::Object>,
  key: &str,
) -> Option<v8::Local<'s, v8::Object>> {
  v8::tc_scope!(tc, scope);
  let key = v8_str(tc, key);
  let desc = obj.get_own_property_descriptor(tc, key.into())?;
  v8::Local::<v8::Object>::try_from(desc).ok()
}

/// `getConstructorName(obj, ctx, recurseTimes, protoProps)`. Returns `None`
/// for null-prototype objects.
fn get_constructor_name<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  obj_in: v8::Local<'s, v8::Value>,
  recurse_times: f64,
  proto_props: &mut Option<Vec<String>>,
) -> R<Option<String>> {
  let tmp = obj_in;
  let mut first_proto: Option<v8::Local<'s, v8::Value>> = None;
  let mut obj = Some(obj_in);

  while let Some(cur) = obj {
    if !(cur.is_object() || cur.is_function() || is_undetectable(scope, cur)) {
      break;
    }
    // Well-known prototype check.
    let well_known_list = intr.well_known(scope);
    let well_known = well_known_list
      .iter()
      .find(|(proto, _, _)| {
        let proto_val: v8::Local<v8::Value> = (*proto).into();
        proto_val.strict_equals(cur)
      })
      .map(|(_, name, ctor)| (name.to_string(), *ctor));
    if let Some((name, constructor)) = well_known {
      let is_instance = {
        v8::tc_scope!(tc, scope);
        tmp.instance_of(tc, constructor.into()).unwrap_or(false)
      };
      if is_instance {
        if proto_props.is_some()
          && first_proto.map(|p| !p.strict_equals(cur)).unwrap_or(true)
        {
          let start_obj = first_proto.unwrap_or(tmp);
          let mut output = std::mem::take(proto_props).unwrap_or_default();
          add_prototype_properties(
            scope,
            intr,
            ctx,
            tmp,
            start_obj,
            recurse_times,
            &mut output,
          )?;
          *proto_props = Some(output);
        }
        return Ok(Some(name));
      }
    }

    let cur_obj = v8::Local::<v8::Object>::try_from(cur).ok();
    let descriptor =
      cur_obj.and_then(|o| get_own_descriptor(scope, o, "constructor"));
    if let Some(desc) = descriptor {
      let desc_value = try_get_str(scope, desc, "value");
      if let Some(desc_value) = desc_value {
        if desc_value.is_function() {
          let f = v8::Local::<v8::Function>::try_from(desc_value).unwrap();
          let f_obj: v8::Local<v8::Object> = f.into();
          let name = try_get_str(scope, f_obj, "name")
            .map(|v| v.to_rust_string_lossy(scope))
            .unwrap_or_default();
          if !name.is_empty() {
            let proto = try_get_str(scope, f_obj, "prototype");
            let is_inst =
              proto.map(|p| is_instanceof(scope, p, tmp)).unwrap_or(false);
            if is_inst {
              if proto_props.is_some() {
                let first_differs =
                  first_proto.map(|p| !p.strict_equals(cur)).unwrap_or(true);
                let builtins = built_in_objects(scope);
                if first_differs || !builtins.iter().any(|b| *b == name) {
                  let start_obj = first_proto.unwrap_or(tmp);
                  let mut output =
                    std::mem::take(proto_props).unwrap_or_default();
                  add_prototype_properties(
                    scope,
                    intr,
                    ctx,
                    tmp,
                    start_obj,
                    recurse_times,
                    &mut output,
                  )?;
                  *proto_props = Some(output);
                }
              }
              return Ok(Some(name));
            }
          }
        }
      }
    }

    let proto = cur_obj.and_then(|o| o.get_prototype(scope));
    obj = match proto {
      Some(p) if !p.is_null() => Some(p),
      _ => None,
    };
    if first_proto.is_none() {
      // JS records `firstProto = obj` after the reassignment, including null.
      first_proto = Some(match proto {
        Some(p) => p,
        None => v8::null(scope).into(),
      });
    }
  }

  if let Some(fp) = first_proto {
    if fp.is_null() {
      return Ok(None);
    }
  } else {
    // While loop never ran (shouldn't happen for objects); treat like a
    // complex prototype below.
  }

  let res = v8::Local::<v8::Object>::try_from(tmp)
    .map(|o| o.get_constructor_name().to_rust_string_lossy(scope))
    .unwrap_or_default();

  if let Some(depth) = ctx.depth {
    if recurse_times > depth {
      return Ok(Some(format!("{res} <Complex prototype>")));
    }
  }

  let first_proto = first_proto.unwrap_or_else(|| v8::null(scope).into());
  let proto_constr = get_constructor_name(
    scope,
    intr,
    ctx,
    first_proto,
    recurse_times + 1.0,
    proto_props,
  )?;

  match proto_constr {
    None => {
      // `${res} <${inspect(firstProto, {...ctx, customInspect: false,
      // depth: -1})}>` — `inspect()` re-applies its option post-processing:
      // iterableLimit overrides maxArrayLength, and colors re-derives the
      // default theme stylize.
      let mut sub_ctx = fork_ctx(ctx);
      sub_ctx.custom_inspect = false;
      sub_ctx.depth = Some(-1.0);
      sub_ctx.max_array_length = sub_ctx.iterable_limit;
      if let Some(n) = sub_ctx.str_abbreviate_size {
        sub_ctx.max_string_length = n;
      }
      if sub_ctx.colors {
        let styles = intr.styles_obj(scope);
        let colors = intr.colors_obj(scope);
        sub_ctx.stylize = StylizeKind::Theme {
          styles,
          colors,
          js_fn: None,
        };
        sub_ctx.stylize_js_fn = None;
      }
      let inner =
        format_value(scope, intr, &mut sub_ctx, first_proto, 0.0, false)?;
      merge_ctx_state(ctx, sub_ctx);
      Ok(Some(format!("{res} <{inner}>")))
    }
    Some(proto_constr) => Ok(Some(format!("{res} <{proto_constr}>"))),
  }
}

/// Clone the context for a nested `inspect()`-style call that shares
/// seen/circular/budget state.
fn fork_ctx<'s>(ctx: &Ctx<'s>) -> Ctx<'s> {
  Ctx {
    show_hidden: ctx.show_hidden,
    depth: ctx.depth,
    colors: ctx.colors,
    custom_inspect: ctx.custom_inspect,
    show_proxy: ctx.show_proxy,
    max_array_length: ctx.max_array_length,
    max_string_length: ctx.max_string_length,
    break_length: ctx.break_length,
    escape_sequences: ctx.escape_sequences,
    compact: ctx.compact,
    sorted: ctx.sorted,
    getters: ctx.getters,
    quotes: ctx.quotes.clone(),
    iterable_limit: ctx.iterable_limit,
    trailing_comma: ctx.trailing_comma,
    indent_level: ctx.indent_level,
    str_abbreviate_size: ctx.str_abbreviate_size,
    indentation_lvl: ctx.indentation_lvl,
    current_depth: ctx.current_depth,
    seen: ctx.seen.clone(),
    circular: ctx.circular.clone(),
    circular_set: ctx.circular_set,
    budget: ctx.budget.clone(),
    stylize: ctx.stylize,
    user_options: ctx.user_options,
    numeric_separator: ctx.numeric_separator,
    ctx_inspect_fn: ctx.ctx_inspect_fn,
    url_prototype: ctx.url_prototype,
    circular_error_message: ctx.circular_error_message.clone(),
    stylize_js_fn: ctx.stylize_js_fn,
    theme_memo: ctx.theme_memo.clone(),
  }
}

fn merge_ctx_state<'s>(ctx: &mut Ctx<'s>, sub: Ctx<'s>) {
  ctx.circular = sub.circular;
  ctx.circular_set = sub.circular_set;
  ctx.url_prototype = sub.url_prototype;
  ctx.circular_error_message = sub.circular_error_message;
}

// ---------------------------------------------------------------------------
// formatRaw

struct FormatterKind<'s> {
  /// What to run to produce the leading output entries.
  kind: FormatterId,
  bound_value: Option<v8::Local<'s, v8::Value>>,
  bound_size: usize,
  braces: (String, String),
}

#[derive(PartialEq, Clone, Copy)]
enum FormatterId {
  None,
  Array,
  Set,
  Map,
  TypedArray,
  MapIterator,
  SetIterator,
  Promise,
  WeakSet,
  WeakMap,
  WeakCollection,
  NamespaceObject,
  ArrayBuffer,
}

fn well_known_symbol_iterator<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Symbol> {
  v8::Symbol::get_iterator(scope)
}

#[allow(clippy::too_many_arguments)]
fn format_raw<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
  typed_array_marker: bool,
  proxy_details: Option<(v8::Local<'s, v8::Value>, v8::Local<'s, v8::Value>)>,
) -> R<String> {
  let value_obj = v8::Local::<v8::Object>::try_from(value)
    .expect("format_raw requires an object");

  let mut proto_props: Option<Vec<String>> = None;
  let within_depth = match ctx.depth {
    None => true,
    Some(d) => recurse_times <= d,
  };
  if ctx.show_hidden && within_depth {
    proto_props = Some(Vec::new());
  }

  let constructor = get_constructor_name(
    scope,
    intr,
    ctx,
    value,
    recurse_times,
    &mut proto_props,
  )?;
  // Reset the variable to check for this later on.
  if let Some(props) = &proto_props {
    if props.is_empty() {
      proto_props = None;
    }
  }

  let mut tag = String::new();
  if proxy_details.is_none() {
    let tag_symbol = v8::Symbol::get_to_string_tag(scope);
    let tag_value = try_get(scope, value_obj, tag_symbol.into());
    if let Some(tag_value) = tag_value {
      if tag_value.is_string() {
        tag = tag_value.to_rust_string_lossy(scope);
      }
    }
  }

  let mut base = String::new();
  let mut formatter = FormatterKind {
    kind: FormatterId::None,
    bound_value: None,
    bound_size: 0,
    braces: ("{".to_string(), "}".to_string()),
  };
  let mut keys: Vec<v8::Local<'s, v8::Value>> = Vec::new();
  let mut extras_type = K_OBJECT_TYPE;
  let only_enumerable = !ctx.show_hidden;

  if proxy_details.is_some() && ctx.show_proxy {
    // `Proxy ` + formatValue(ctx, proxyDetails, recurseTimes)
    let (target, handler) = proxy_details.unwrap();
    let arr = v8::Array::new_with_elements(scope, &[target, handler]);
    let inner =
      format_value(scope, intr, ctx, arr.into(), recurse_times, false)?;
    return Ok(format!("Proxy {inner}"));
  }

  // Iterators and the rest are split to reduce checks.
  let mut no_iterator = true;
  let iterator_symbol = well_known_symbol_iterator(scope);
  let has_iterator = {
    v8::tc_scope!(tc, scope);
    value_obj.has(tc, iterator_symbol.into()).unwrap_or(false)
  };
  if has_iterator || constructor.is_none() {
    no_iterator = false;
    if value.is_array() {
      let arr = v8::Local::<v8::Array>::try_from(value).unwrap();
      let length = arr.length();
      let prefix = if constructor.as_deref() != Some("Array") || !tag.is_empty()
      {
        get_prefix(
          constructor.as_deref(),
          &tag,
          "Array",
          &format!("({length})"),
        )
      } else {
        String::new()
      };
      keys = get_non_index_property_names(scope, value_obj, only_enumerable);
      formatter.braces = (format!("{prefix}["), "]".to_string());
      if length == 0 && keys.is_empty() && proto_props.is_none() {
        return Ok(format!("{}]", formatter.braces.0));
      }
      extras_type = K_ARRAY_EXTRAS_TYPE;
      formatter.kind = FormatterId::Array;
    } else if value.is_set() {
      let set = v8::Local::<v8::Set>::try_from(value).unwrap();
      let size = set.size();
      let prefix =
        get_prefix(constructor.as_deref(), &tag, "Set", &format!("({size})"));
      keys = get_keys(scope, intr, value_obj, ctx.show_hidden);
      formatter.kind = FormatterId::Set;
      formatter.bound_value = Some(value);
      if size == 0 && keys.is_empty() && proto_props.is_none() {
        return Ok(format!("{prefix}{{}}"));
      }
      formatter.braces = (format!("{prefix}{{"), "}".to_string());
    } else if value.is_map() {
      let map = v8::Local::<v8::Map>::try_from(value).unwrap();
      let size = map.size();
      let prefix =
        get_prefix(constructor.as_deref(), &tag, "Map", &format!("({size})"));
      keys = get_keys(scope, intr, value_obj, ctx.show_hidden);
      formatter.kind = FormatterId::Map;
      formatter.bound_value = Some(value);
      if size == 0 && keys.is_empty() && proto_props.is_none() {
        return Ok(format!("{prefix}{{}}"));
      }
      formatter.braces = (format!("{prefix}{{"), "}".to_string());
    } else if value.is_typed_array() {
      let ta = v8::Local::<v8::TypedArray>::try_from(value).unwrap();
      keys = get_non_index_property_names(scope, value_obj, only_enumerable);
      let size = ta.length();
      let fallback = "";
      let prefix = get_prefix(
        constructor.as_deref(),
        &tag,
        fallback,
        &format!("({size})"),
      );
      formatter.braces = (format!("{prefix}["), "]".to_string());
      if size == 0 && keys.is_empty() && !ctx.show_hidden {
        return Ok(format!("{}]", formatter.braces.0));
      }
      formatter.kind = FormatterId::TypedArray;
      formatter.bound_value = Some(value);
      formatter.bound_size = size;
      extras_type = K_ARRAY_EXTRAS_TYPE;
    } else if value.is_map_iterator() {
      keys = get_keys(scope, intr, value_obj, ctx.show_hidden);
      formatter.braces = get_iterator_braces("Map", &tag);
      formatter.kind = FormatterId::MapIterator;
    } else if value.is_set_iterator() {
      keys = get_keys(scope, intr, value_obj, ctx.show_hidden);
      formatter.braces = get_iterator_braces("Set", &tag);
      formatter.kind = FormatterId::SetIterator;
    } else {
      no_iterator = true;
    }
  }
  if no_iterator {
    let use_show_hidden = ctx.show_hidden || value.is_module_namespace_object();
    keys = get_keys(scope, intr, value_obj, use_show_hidden);
    formatter.braces = ("{".to_string(), "}".to_string());
    let constructor_str = constructor.as_deref();
    if constructor_str == Some("Object") {
      if value.is_arguments_object() {
        formatter.braces.0 = "[Arguments] {".to_string();
      } else if !tag.is_empty() {
        formatter.braces.0 =
          format!("{}{{", get_prefix(constructor_str, &tag, "Object", ""));
      }
      if keys.is_empty() && proto_props.is_none() {
        return Ok(format!("{}}}", formatter.braces.0));
      }
    } else if value.is_function() {
      base = get_function_base(scope, intr, value, constructor_str, &tag)?;
      if keys.is_empty() && proto_props.is_none() {
        return ctx.stylize(scope, &base, "special");
      }
    } else if value.is_reg_exp() {
      base = regexp_to_string(scope, intr, value)?;
      let prefix = get_prefix(constructor_str, &tag, "RegExp", "");
      if prefix != "RegExp " {
        base = format!("{prefix}{base}");
      }
      let past_depth = match ctx.depth {
        Some(d) => recurse_times > d,
        None => false,
      };
      if (keys.is_empty() && proto_props.is_none()) || past_depth {
        return ctx.stylize(scope, &base, "regexp");
      }
    } else if value.is_date() {
      let date = v8::Local::<v8::Date>::try_from(value).unwrap();
      let time = date.value_of();
      base = if time.is_nan() {
        date_to_string_invalid()
      } else {
        date_to_iso_string(time)
      };
      let prefix = get_prefix(constructor_str, &tag, "Date", "");
      if prefix != "Date " {
        base = format!("{prefix}{base}");
      }
      if keys.is_empty() && proto_props.is_none() {
        return ctx.stylize(scope, &base, "date");
      }
    } else if is_intl_locale(scope, value) {
      formatter.braces.0 =
        format!("{}{{", get_prefix(constructor_str, &tag, "Intl.Locale", ""));
      let extra_keys = [
        "baseName",
        "calendar",
        "caseFirst",
        "collation",
        "hourCycle",
        "language",
        "numberingSystem",
        "numeric",
        "region",
        "script",
      ];
      let mut new_keys: Vec<v8::Local<'s, v8::Value>> =
        extra_keys.iter().map(|k| v8_str(scope, k).into()).collect();
      new_keys.extend(keys);
      keys = new_keys;
    } else if is_temporal_object(scope, value) {
      // return ctx.stylize(value.toString(), "temporal");
      let s = {
        v8::tc_scope!(tc, scope);
        match value.to_string(tc) {
          Some(s) => s.to_rust_string_lossy(tc),
          None => {
            let exc = tc.exception();
            return Err(grab_err(tc, exc));
          }
        }
      };
      return ctx.stylize(scope, &s, "temporal");
    } else if value.is_native_error() || {
      let error_prototype = intr.error_prototype(scope);
      is_prototype_of(scope, error_prototype.into(), value)
    } {
      base = super::error_fmt::format_error(
        scope,
        intr,
        ctx,
        value_obj,
        constructor.as_deref(),
        &tag,
        &mut keys,
      )?;
      if keys.is_empty() && proto_props.is_none() {
        return Ok(base);
      }
    } else if value.is_array_buffer() || value.is_shared_array_buffer() {
      let array_type = if value.is_array_buffer() {
        "ArrayBuffer"
      } else {
        "SharedArrayBuffer"
      };
      let prefix = get_prefix(constructor_str, &tag, array_type, "");
      if !typed_array_marker {
        formatter.kind = FormatterId::ArrayBuffer;
        formatter.bound_value = Some(value);
      } else if keys.is_empty() && proto_props.is_none() {
        let byte_length = any_array_buffer_byte_length(scope, value);
        let num = v8::Number::new(scope, byte_length as f64);
        let formatted = format_number(scope, ctx, num.into())?;
        return Ok(format!("{prefix}{{ byteLength: {formatted} }}"));
      }
      formatter.braces.0 = format!("{prefix}{{");
      let mut new_keys: Vec<v8::Local<'s, v8::Value>> =
        vec![v8_str(scope, "byteLength").into()];
      new_keys.extend(keys);
      keys = new_keys;
    } else if value.is_data_view() {
      formatter.braces.0 =
        format!("{}{{", get_prefix(constructor_str, &tag, "DataView", ""));
      // .buffer goes last, it's not a primitive like the others.
      let mut new_keys: Vec<v8::Local<'s, v8::Value>> = vec![
        v8_str(scope, "byteLength").into(),
        v8_str(scope, "byteOffset").into(),
        v8_str(scope, "buffer").into(),
      ];
      new_keys.extend(keys);
      keys = new_keys;
    } else if value.is_promise() {
      formatter.braces.0 =
        format!("{}{{", get_prefix(constructor_str, &tag, "Promise", ""));
      formatter.kind = FormatterId::Promise;
      formatter.bound_value = Some(value);
    } else if value.is_weak_set() {
      formatter.braces.0 =
        format!("{}{{", get_prefix(constructor_str, &tag, "WeakSet", ""));
      formatter.kind = if ctx.show_hidden {
        FormatterId::WeakSet
      } else {
        FormatterId::WeakCollection
      };
      formatter.bound_value = Some(value);
    } else if value.is_weak_map() {
      formatter.braces.0 =
        format!("{}{{", get_prefix(constructor_str, &tag, "WeakMap", ""));
      formatter.kind = if ctx.show_hidden {
        FormatterId::WeakMap
      } else {
        FormatterId::WeakCollection
      };
      formatter.bound_value = Some(value);
    } else if value.is_module_namespace_object() {
      formatter.braces.0 =
        format!("{}{{", get_prefix(constructor_str, &tag, "Module", ""));
      formatter.kind = FormatterId::NamespaceObject;
      formatter.bound_value = Some(value);
    } else if is_boxed_primitive(value) {
      base = get_boxed_base(
        scope,
        intr,
        ctx,
        value,
        &mut keys,
        constructor_str,
        &tag,
      )?;
      if keys.is_empty() && proto_props.is_none() {
        return Ok(base);
      }
    } else if is_url_instance(scope, intr, ctx, value)?
      && !match ctx.depth {
        Some(d) => recurse_times > d,
        None => false,
      }
    {
      let href = js_get_str(scope, value_obj, "href")?;
      base = to_rust_string(scope, href);
      if keys.is_empty() && proto_props.is_none() {
        return Ok(base);
      }
    } else {
      if keys.is_empty() && proto_props.is_none() {
        return Ok(format!(
          "{}{{}}",
          get_ctx_style(scope, value, constructor_str, &tag)
        ));
      }
      formatter.braces.0 =
        format!("{}{{", get_ctx_style(scope, value, constructor_str, &tag));
    }
  }

  if let Some(d) = ctx.depth {
    if recurse_times > d {
      let style = get_ctx_style(scope, value, constructor.as_deref(), &tag);
      let mut constructor_name =
        style[..style.len().saturating_sub(1)].to_string();
      if constructor.is_some() {
        constructor_name = format!("[{constructor_name}]");
      }
      return ctx.stylize(scope, &constructor_name, "special");
    }
  }
  let recurse_times = recurse_times + 1.0;

  ctx.seen.push(value);
  ctx.current_depth = recurse_times;

  let mut braces = formatter.braces.clone();

  let output_result: R<Vec<String>> = (|| {
    let mut output = match formatter.kind {
      FormatterId::NamespaceObject => format_namespace_object(
        scope,
        intr,
        ctx,
        &mut keys,
        value,
        recurse_times,
      )?,
      FormatterId::MapIterator | FormatterId::SetIterator => {
        let (entries, is_key_value) = value_obj.preview_entries(scope);
        match entries {
          None => Vec::new(),
          Some(entries) => {
            if is_key_value {
              // Mark entry iterators as such.
              braces.0 = iterator_braces_mark_entries(&braces.0);
              format_map_iter_inner(
                scope,
                intr,
                ctx,
                recurse_times,
                entries,
                K_MAP_ENTRIES,
              )?
            } else {
              format_set_iter_inner(
                scope,
                intr,
                ctx,
                recurse_times,
                entries,
                K_ITERATOR,
              )?
            }
          }
        }
      }
      _ => run_formatter(scope, intr, ctx, &formatter, value, recurse_times)?,
    };
    for key in &keys {
      output.push(format_property(
        scope,
        intr,
        ctx,
        value,
        recurse_times,
        *key,
        extras_type,
        None,
        None,
      )?);
    }
    if let Some(props) = proto_props.take() {
      output.extend(props);
    }
    Ok(output)
  })();

  let mut output = match output_result {
    Ok(output) => output,
    Err(err) => {
      ctx.seen.pop();
      let stack = error_stack_string(scope, &err);
      return ctx.stylize(
        scope,
        &format!("[Internal Formatting Error] {stack}"),
        "internalError",
      );
    }
  };
  if ctx.circular_set {
    let index = ctx
      .circular
      .iter()
      .find(|(v, _)| v.strict_equals(value))
      .map(|(_, i)| *i);
    if let Some(index) = index {
      let reference =
        ctx.stylize(scope, &format!("<ref *{index}>"), "special")?;
      if !ctx.compact.is_true() {
        base = if base.is_empty() {
          reference
        } else {
          format!("{reference} {base}")
        };
      } else {
        braces.0 = format!("{reference} {}", braces.0);
      }
    }
  }
  ctx.seen.pop();

  if !matches!(ctx.sorted, Sorted::No) {
    if extras_type == K_OBJECT_TYPE {
      sort_output(scope, ctx, &mut output)?;
    } else if keys.len() > 1 {
      let tail_start = output.len() - keys.len();
      let mut tail: Vec<String> = output[tail_start..].to_vec();
      sort_output(scope, ctx, &mut tail)?;
      output.splice(tail_start.., tail);
    }
  }

  let res = reduce_to_single_string(
    scope,
    ctx,
    output,
    &base,
    &braces,
    extras_type,
    recurse_times,
    Some(value),
  )?;
  let budget = ctx.budget.get(&ctx.indentation_lvl).copied().unwrap_or(0);
  let new_length = budget + res.len();
  ctx.budget.insert(ctx.indentation_lvl, new_length);
  if new_length > (1 << 27) {
    ctx.depth = Some(-1.0);
  }
  Ok(res)
}

fn sort_output<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  ctx: &mut Ctx<'s>,
  output: &mut [String],
) -> R<()> {
  match ctx.sorted {
    Sorted::No => {}
    Sorted::Yes => {
      output.sort_by(|a, b| {
        let a16: Vec<u16> = a.encode_utf16().collect();
        let b16: Vec<u16> = b.encode_utf16().collect();
        a16.cmp(&b16)
      });
    }
    Sorted::Comparator(f) => {
      // Call the user comparator pairwise (insertion-sort style to bound
      // comparator calls like Array.prototype.sort would).
      let mut err = None;
      let mut items: Vec<String> = output.to_vec();
      items.sort_by(|a, b| {
        if err.is_some() {
          return std::cmp::Ordering::Equal;
        }
        let a_v = v8_str(scope, a);
        let b_v = v8_str(scope, b);
        let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
        match js_call(scope, f, undef, &[a_v.into(), b_v.into()]) {
          Ok(ret) => {
            let n = ret.number_value(scope).unwrap_or(f64::NAN);
            if n < 0.0 {
              std::cmp::Ordering::Less
            } else if n > 0.0 {
              std::cmp::Ordering::Greater
            } else {
              std::cmp::Ordering::Equal
            }
          }
          Err(e) => {
            err = Some(e);
            std::cmp::Ordering::Equal
          }
        }
      });
      if let Some(e) = err {
        return Err(e);
      }
      output.clone_from_slice(&items);
    }
  }
  Ok(())
}

pub fn error_stack_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  err: &JsErr,
) -> String {
  let exc = v8::Local::new(scope, &err.0);
  if let Ok(obj) = v8::Local::<v8::Object>::try_from(exc) {
    if let Some(stack) = try_get_str(scope, obj, "stack") {
      if !stack.is_undefined() {
        return stack.to_rust_string_lossy(scope);
      }
    }
  }
  exc.to_rust_string_lossy(scope)
}

fn get_iterator_braces(ty: &str, tag: &str) -> (String, String) {
  let expected = format!("{ty} Iterator");
  let mut tag = tag.to_string();
  if tag != expected {
    if !tag.is_empty() {
      tag.push_str("] [");
    }
    tag.push_str(&expected);
  }
  (format!("[{tag}] {{"), "}".to_string())
}

fn is_boxed_primitive(value: v8::Local<v8::Value>) -> bool {
  value.is_number_object()
    || value.is_string_object()
    || value.is_boolean_object()
    || value.is_big_int_object()
    || value.is_symbol_object()
}

fn is_intl_locale<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> bool {
  let context = scope.get_current_context();
  let global = context.global(scope);
  let Some(intl) = try_get_str(scope, global, "Intl") else {
    return false;
  };
  let Ok(intl) = v8::Local::<v8::Object>::try_from(intl) else {
    return false;
  };
  let Some(locale) = try_get_str(scope, intl, "Locale") else {
    return false;
  };
  let Ok(locale) = v8::Local::<v8::Object>::try_from(locale) else {
    return false;
  };
  let Some(proto) = try_get_str(scope, locale, "prototype") else {
    return false;
  };
  is_prototype_of(scope, proto, value)
}

fn is_temporal_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> bool {
  let context = scope.get_current_context();
  let global = context.global(scope);
  let Some(temporal) = try_get_str(scope, global, "Temporal") else {
    return false;
  };
  if temporal.is_undefined() {
    return false;
  }
  let Ok(temporal) = v8::Local::<v8::Object>::try_from(temporal) else {
    return false;
  };
  for name in [
    "Instant",
    "ZonedDateTime",
    "PlainDate",
    "PlainTime",
    "PlainDateTime",
    "PlainYearMonth",
    "PlainMonthDay",
    "Duration",
  ] {
    let Some(class) = try_get_str(scope, temporal, name) else {
      continue;
    };
    let Ok(class) = v8::Local::<v8::Object>::try_from(class) else {
      continue;
    };
    let Some(proto) = try_get_str(scope, class, "prototype") else {
      continue;
    };
    if is_prototype_of(scope, proto, value) {
      return true;
    }
  }
  false
}

fn is_url_instance<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
) -> R<bool> {
  let proto = match ctx.url_prototype {
    Some(memo) => memo,
    None => {
      let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
      let ret = {
        let f = intr.get_url_prototype(scope);
        js_call(scope, f, undef, &[])
      }?;
      let proto = v8::Local::<v8::Object>::try_from(ret).ok();
      ctx.url_prototype = Some(proto);
      proto
    }
  };
  match proto {
    Some(proto) => Ok(is_prototype_of(scope, proto.into(), value)),
    None => Ok(false),
  }
}

fn any_array_buffer_byte_length<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> usize {
  let _ = scope;
  if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
    return ab.byte_length();
  }
  if let Ok(sab) = v8::Local::<v8::SharedArrayBuffer>::try_from(value) {
    return sab.byte_length();
  }
  0
}

fn regexp_to_string<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  regexp: v8::Local<'s, v8::Value>,
) -> R<String> {
  let ret = {
    let f = intr.reg_exp_to_string(scope);
    js_call(scope, f, regexp, &[])
  }?;
  Ok(to_rust_string(scope, ret))
}

// ---------------------------------------------------------------------------
// dates

/// Days-to-civil (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
  let z = z + 719468;
  let era = if z >= 0 { z } else { z - 146096 } / 146097;
  let doe = (z - era * 146097) as u64; // [0, 146096]
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
  let y = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
  let mp = (5 * doy + 2) / 153; // [0, 11]
  let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
  let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
  (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `Date.prototype.toISOString` for a finite epoch-milliseconds value.
pub fn date_to_iso_string(time: f64) -> String {
  let ms_total = time.floor() as i64;
  let days = ms_total.div_euclid(86_400_000);
  let ms_of_day = ms_total.rem_euclid(86_400_000);
  let (year, month, day) = civil_from_days(days);
  let hours = ms_of_day / 3_600_000;
  let minutes = (ms_of_day % 3_600_000) / 60_000;
  let seconds = (ms_of_day % 60_000) / 1000;
  let ms = ms_of_day % 1000;
  let year_str = if (0..=9999).contains(&year) {
    format!("{year:04}")
  } else if year < 0 {
    format!("-{:06}", -year)
  } else {
    format!("+{year:06}")
  };
  format!(
    "{year_str}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{ms:03}Z"
  )
}

pub fn date_to_string_invalid() -> String {
  "Invalid Date".to_string()
}

// ---------------------------------------------------------------------------
// function/class/boxed bases

fn get_class_base<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
  constructor: Option<&str>,
  tag: &str,
) -> R<String> {
  let value_obj = v8::Local::<v8::Object>::try_from(value).unwrap();
  let has_name = {
    v8::tc_scope!(tc, scope);
    let key = v8_str(tc, "name");
    value_obj.has_own_property(tc, key.into()).unwrap_or(false)
  };
  let name_val = if has_name {
    js_get_str(scope, value_obj, "name")?
  } else {
    v8::undefined(scope).into()
  };
  let name_truthy = has_name && name_val.boolean_value(scope);
  let name = if name_truthy {
    name_val.to_rust_string_lossy(scope)
  } else {
    "(anonymous)".to_string()
  };
  let mut base = format!("class {name}");
  if let Some(ctor) = constructor {
    if ctor != "Function" {
      base.push_str(&format!(" [{ctor}]"));
    }
  }
  if !tag.is_empty() && constructor != Some(tag) {
    base.push_str(&format!(" [{tag}]"));
  }
  if constructor.is_some() {
    let proto = value_obj.get_prototype(scope);
    if let Some(proto) = proto {
      if let Ok(proto_obj) = v8::Local::<v8::Object>::try_from(proto) {
        let super_name = try_get_str(scope, proto_obj, "name")
          .filter(|v| v.is_string())
          .map(|v| v.to_rust_string_lossy(scope))
          .unwrap_or_default();
        if !super_name.is_empty() {
          base.push_str(&format!(" extends {super_name}"));
        }
      }
    }
  } else {
    base.push_str(" extends [null prototype]");
  }
  Ok(format!("[{base}]"))
}

/// Strip `//`-to-newline and `/* */` comments, replacing each with the
/// string "undefined" (a faithful port of a quirk in 01_console.js where
/// `RegExpPrototypeSymbolReplace` is called without a replacement value).
fn strip_comments_js_quirk(s: &str) -> String {
  let bytes = s.as_bytes();
  let mut out = String::with_capacity(s.len());
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
      // `//.*?\n` — must find a newline; otherwise no match.
      if let Some(nl) = s[i..].find('\n') {
        out.push_str("undefined");
        i += nl + 1;
        continue;
      }
    }
    if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
      if let Some(end) = s[i + 2..].find("*/") {
        out.push_str("undefined");
        i += 2 + end + 2;
        continue;
      }
    }
    // Append the full UTF-8 char.
    let ch = s[i..].chars().next().unwrap();
    out.push(ch);
    i += ch.len_utf8();
  }
  out
}

/// `classRegExp` = `^(\s+[^(]*?)\s*{`: the text must start with whitespace
/// and contain no `(` before the first `{`.
fn matches_class_regexp(s: &str) -> bool {
  let mut chars = s.chars();
  match chars.next() {
    Some(c) if c.is_whitespace() => {}
    _ => return false,
  }
  for c in s.chars().skip(1) {
    match c {
      '{' => return true,
      '(' => return false,
      _ => {}
    }
  }
  false
}

fn get_function_base<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  value: v8::Local<'s, v8::Value>,
  constructor: Option<&str>,
  tag: &str,
) -> R<String> {
  let stringified_val = {
    let f = intr.function_to_string(scope);
    js_call(scope, f, value, &[])
  }?;
  let stringified = to_rust_string(scope, stringified_val);
  if stringified.starts_with("class") && stringified.ends_with('}') {
    let slice = &stringified[5..stringified.len() - 1];
    if let Some(bracket_index) = slice.find('{') {
      if !slice[..bracket_index].contains('(')
        || matches_class_regexp(&strip_comments_js_quirk(slice))
      {
        return get_class_base(scope, value, constructor, tag);
      }
    }
  }
  let mut ty = "Function".to_string();
  if value.is_generator_function() {
    ty = format!("Generator{ty}");
  }
  if value.is_async_function() {
    ty = format!("Async{ty}");
  }
  let mut base = format!("[{ty}");
  if constructor.is_none() {
    base.push_str(" (null prototype)");
  }
  let value_obj = v8::Local::<v8::Object>::try_from(value).unwrap();
  let name = js_get_str(scope, value_obj, "name")?;
  let name_str = if name.is_string() {
    name.to_rust_string_lossy(scope)
  } else {
    // `value.name === ""` is false for non-strings; the template coerces.
    name.to_rust_string_lossy(scope)
  };
  let name_is_empty_string = name.is_string() && name_str.is_empty();
  if name_is_empty_string {
    base.push_str(" (anonymous)");
  } else {
    base.push_str(&format!(": {name_str}"));
  }
  base.push(']');
  if let Some(ctor) = constructor {
    if ctor != ty {
      base.push_str(&format!(" {ctor}"));
    }
  }
  if !tag.is_empty() && constructor != Some(tag) {
    base.push_str(&format!(" [{tag}]"));
  }
  Ok(base)
}

fn get_boxed_base<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  keys: &mut Vec<v8::Local<'s, v8::Value>>,
  constructor: Option<&str>,
  tag: &str,
) -> R<String> {
  let (ty, primitive): (&str, v8::Local<'s, v8::Value>) =
    if value.is_number_object() {
      let n = {
        let f = intr.number_value_of(scope);
        js_call(scope, f, value, &[])
      }?;
      ("Number", n)
    } else if value.is_string_object() {
      let s = {
        let f = intr.string_value_of(scope);
        js_call(scope, f, value, &[])
      }?;
      // For boxed Strings remove the 0-n indexed entries.
      let len = v8::Local::<v8::String>::try_from(s)
        .map(|s| s.length())
        .unwrap_or(0);
      let drain = len.min(keys.len());
      keys.drain(0..drain);
      ("String", s)
    } else if value.is_boolean_object() {
      let b = {
        let f = intr.boolean_value_of(scope);
        js_call(scope, f, value, &[])
      }?;
      ("Boolean", b)
    } else if value.is_big_int_object() {
      let b = {
        let f = intr.big_int_value_of(scope);
        js_call(scope, f, value, &[])
      }?;
      ("BigInt", b)
    } else {
      let s = {
        let f = intr.symbol_value_of(scope);
        js_call(scope, f, value, &[])
      }?;
      ("Symbol", s)
    };

  let mut base = format!("[{ty}");
  if Some(ty) != constructor {
    match constructor {
      None => base.push_str(" (null prototype)"),
      Some(c) => base.push_str(&format!(" ({c})")),
    }
  }
  let inner = format_primitive(scope, ctx, primitive, true)?;
  base.push_str(&format!(": {inner}]"));
  if !tag.is_empty() && Some(tag) != constructor {
    base.push_str(&format!(" [{tag}]"));
  }
  if !keys.is_empty() || matches!(ctx.stylize, StylizeKind::NoColor) {
    return Ok(base);
  }
  let flavour: &'static str = match ty {
    "Number" => "number",
    "String" => "string",
    "Boolean" => "boolean",
    "BigInt" => "bigint",
    _ => "symbol",
  };
  ctx.stylize(scope, &base, flavour)
}

// ---------------------------------------------------------------------------
// collection formatters

fn run_formatter<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  formatter: &FormatterKind<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  match formatter.kind {
    FormatterId::None => Ok(Vec::new()),
    FormatterId::Array => format_array(scope, intr, ctx, value, recurse_times),
    FormatterId::Set => format_set(
      scope,
      intr,
      ctx,
      formatter.bound_value.unwrap(),
      recurse_times,
    ),
    FormatterId::Map => format_map(
      scope,
      intr,
      ctx,
      formatter.bound_value.unwrap(),
      recurse_times,
    ),
    FormatterId::TypedArray => format_typed_array(
      scope,
      intr,
      ctx,
      formatter.bound_value.unwrap(),
      formatter.bound_size,
      recurse_times,
    ),
    FormatterId::MapIterator | FormatterId::SetIterator => {
      unreachable!("iterators are special-cased before run_formatter")
    }
    FormatterId::Promise => format_promise(
      scope,
      intr,
      ctx,
      formatter.bound_value.unwrap(),
      recurse_times,
    ),
    FormatterId::WeakSet => format_weak_set(
      scope,
      intr,
      ctx,
      formatter.bound_value.unwrap(),
      recurse_times,
    ),
    FormatterId::WeakMap => format_weak_map(
      scope,
      intr,
      ctx,
      formatter.bound_value.unwrap(),
      recurse_times,
    ),
    FormatterId::WeakCollection => {
      Ok(vec![ctx.stylize(scope, "<items unknown>", "special")?])
    }
    FormatterId::NamespaceObject => {
      // keys are consumed by the namespace formatter in the caller.
      unreachable!("namespace objects are special-cased before run_formatter")
    }
    FormatterId::ArrayBuffer => {
      format_array_buffer(scope, ctx, formatter.bound_value.unwrap())
    }
  }
}

fn more_items(remaining: usize) -> String {
  format!(
    "... {} more item{}",
    remaining,
    if remaining > 1 { "s" } else { "" }
  )
}

fn format_array<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  let arr = v8::Local::<v8::Array>::try_from(value).unwrap();
  let value_obj: v8::Local<v8::Object> = arr.into();
  let val_len = arr.length() as usize;
  let len = (ctx.max_array_length.max(0.0) as usize).min(val_len);
  let remaining = val_len - len;
  let mut output = Vec::new();
  for i in 0..len {
    let has_own = {
      v8::tc_scope!(tc, scope);
      let key = v8_str(tc, &i.to_string());
      value_obj.has_own_property(tc, key.into()).unwrap_or(false)
    };
    if !has_own {
      return format_special_array(
        scope,
        intr,
        ctx,
        value,
        recurse_times,
        len,
        output,
        i,
      );
    }
    let key = v8::Number::new(scope, i as f64);
    output.push(format_property(
      scope,
      intr,
      ctx,
      value,
      recurse_times,
      key.into(),
      K_ARRAY_TYPE,
      None,
      None,
    )?);
  }
  if remaining > 0 {
    output.push(more_items(remaining));
  }
  Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn format_special_array<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
  max_length: usize,
  mut output: Vec<String>,
  start: usize,
) -> R<Vec<String>> {
  let arr = v8::Local::<v8::Array>::try_from(value).unwrap();
  let value_obj: v8::Local<v8::Object> = arr.into();
  // ObjectKeys(value)
  let keys: Vec<String> = {
    v8::tc_scope!(tc, scope);
    match value_obj.get_property_names(
      tc,
      v8::GetPropertyNamesArgs {
        mode: v8::KeyCollectionMode::OwnOnly,
        property_filter: v8::PropertyFilter::ONLY_ENUMERABLE
          | v8::PropertyFilter::SKIP_SYMBOLS,
        index_filter: v8::IndexFilter::IncludeIndices,
        key_conversion: v8::KeyConversionMode::ConvertToString,
      },
    ) {
      Some(names) => {
        let mut out = Vec::with_capacity(names.length() as usize);
        for i in 0..names.length() {
          if let Some(v) = names.get_index(tc, i) {
            out.push(v.to_rust_string_lossy(tc));
          }
        }
        out
      }
      None => Vec::new(),
    }
  };
  let mut index = start;
  let mut i = start;
  while i < keys.len() && output.len() < max_length {
    let key = &keys[i];
    let tmp: f64 = key.parse().unwrap_or(f64::NAN);
    // Arrays can only have up to 2^32 - 1 entries
    if tmp > (u32::MAX - 1) as f64 {
      break;
    }
    if index.to_string() != *key {
      if !quote::is_canonical_index(key) {
        break;
      }
      let empty_items = tmp as usize - index;
      let ending = if empty_items > 1 { "s" } else { "" };
      let message = format!("<{empty_items} empty item{ending}>");
      output.push(ctx.stylize(scope, &message, "undefined")?);
      index = tmp as usize;
      if output.len() == max_length {
        break;
      }
    }
    let key_v8 = v8_str(scope, key);
    output.push(format_property(
      scope,
      intr,
      ctx,
      value,
      recurse_times,
      key_v8.into(),
      K_ARRAY_TYPE,
      None,
      None,
    )?);
    index += 1;
    i += 1;
  }
  let remaining = arr.length() as i64 - index as i64;
  if output.len() != max_length {
    if remaining > 0 {
      let ending = if remaining > 1 { "s" } else { "" };
      let message = format!("<{remaining} empty item{ending}>");
      output.push(ctx.stylize(scope, &message, "undefined")?);
    }
  } else if remaining > 0 {
    output.push(more_items(remaining as usize));
  }
  Ok(output)
}

fn format_set<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  ctx.indentation_lvl += 2;
  let set = v8::Local::<v8::Set>::try_from(value).unwrap();
  let entries = set.as_array(scope);
  let val_len = set.size() as usize;
  let len = (ctx.iterable_limit.max(0.0) as usize).min(val_len);
  let remaining = val_len - len;
  let mut output = Vec::new();
  for i in 0..len {
    let Some(item) = entries.get_index(scope, i as u32) else {
      continue;
    };
    output.push(format_value(scope, intr, ctx, item, recurse_times, false)?);
  }
  if remaining > 0 {
    output.push(more_items(remaining));
  }
  ctx.indentation_lvl -= 2;
  Ok(output)
}

fn format_map<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  ctx.indentation_lvl += 2;
  let map = v8::Local::<v8::Map>::try_from(value).unwrap();
  let entries = map.as_array(scope); // [k0, v0, k1, v1, ...]
  let val_len = map.size() as usize;
  let len = (ctx.iterable_limit.max(0.0) as usize).min(val_len);
  let remaining = val_len - len;
  let mut output = Vec::new();
  for i in 0..len {
    let Some(k) = entries.get_index(scope, (i * 2) as u32) else {
      continue;
    };
    let Some(v) = entries.get_index(scope, (i * 2 + 1) as u32) else {
      continue;
    };
    let k_str = format_value(scope, intr, ctx, k, recurse_times, false)?;
    let v_str = format_value(scope, intr, ctx, v, recurse_times, false)?;
    output.push(format!("{k_str} => {v_str}"));
  }
  if remaining > 0 {
    output.push(more_items(remaining));
  }
  ctx.indentation_lvl -= 2;
  Ok(output)
}

fn format_typed_array<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  length: usize,
  recurse_times: f64,
) -> R<Vec<String>> {
  let value_obj = v8::Local::<v8::Object>::try_from(value).unwrap();
  let max_length = (ctx.max_array_length.max(0.0) as usize).min(length);
  let remaining = length - max_length;
  let mut output = Vec::with_capacity(max_length);
  let is_bigint_array =
    value.is_big_int64_array() || value.is_big_uint64_array();
  for i in 0..max_length {
    let Some(elem) = value_obj.get_index(scope, i as u32) else {
      continue;
    };
    let s = if is_bigint_array {
      format_bigint(scope, ctx, elem)?
    } else {
      format_number(scope, ctx, elem)?
    };
    output.push(s);
  }
  if remaining > 0 {
    output.push(more_items(remaining));
  }
  if ctx.show_hidden {
    // .buffer goes last, it's not a primitive like the others.
    ctx.indentation_lvl += 2;
    for key in [
      "BYTES_PER_ELEMENT",
      "length",
      "byteLength",
      "byteOffset",
      "buffer",
    ] {
      let v = js_get_str(scope, value_obj, key)?;
      let str_ = format_value(scope, intr, ctx, v, recurse_times, true)?;
      output.push(format!("[{key}]: {str_}"));
    }
    ctx.indentation_lvl -= 2;
  }
  Ok(output)
}

/// Marks entry iterators in the braces (mirrors formatIterator mutating
/// braces[0]).
fn iterator_braces_mark_entries(braces0: &str) -> String {
  if let Some(stripped) = braces0.strip_suffix(" Iterator] {") {
    format!("{stripped} Entries] {{")
  } else {
    braces0.to_string()
  }
}

fn format_promise<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  let promise = v8::Local::<v8::Promise>::try_from(value).unwrap();
  match promise.state() {
    v8::PromiseState::Pending => {
      Ok(vec![ctx.stylize(scope, "<pending>", "special")?])
    }
    state => {
      let result = promise.result(scope);
      ctx.indentation_lvl += 2;
      let str_ = format_value(scope, intr, ctx, result, recurse_times, false)?;
      ctx.indentation_lvl -= 2;
      Ok(vec![if state == v8::PromiseState::Rejected {
        let rejected = ctx.stylize(scope, "<rejected>", "special")?;
        format!("{rejected} {str_}")
      } else {
        str_
      }])
    }
  }
}

fn format_weak_set<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  let value_obj = v8::Local::<v8::Object>::try_from(value).unwrap();
  let (entries, _) = value_obj.preview_entries(scope);
  let Some(entries) = entries else {
    return Ok(Vec::new());
  };
  format_set_iter_inner(scope, intr, ctx, recurse_times, entries, K_WEAK)
}

fn format_weak_map<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  let value_obj = v8::Local::<v8::Object>::try_from(value).unwrap();
  let (entries, _) = value_obj.preview_entries(scope);
  let Some(entries) = entries else {
    return Ok(Vec::new());
  };
  format_map_iter_inner(scope, intr, ctx, recurse_times, entries, K_WEAK)
}

fn format_map_iter_inner<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  recurse_times: f64,
  entries: v8::Local<'s, v8::Array>,
  state: u8,
) -> R<Vec<String>> {
  let max_array_length = ctx.max_array_length.max(0.0);
  // Entries exist as [key1, val1, key2, val2, ...]
  let len = entries.length() as usize / 2;
  let remaining = len as i64 - max_array_length as i64;
  let max_length = (max_array_length as usize).min(len);
  let mut output = Vec::with_capacity(max_length);
  ctx.indentation_lvl += 2;
  if state == K_WEAK {
    for i in 0..max_length {
      let pos = (i * 2) as u32;
      let k = entries.get_index(scope, pos).unwrap();
      let v = entries.get_index(scope, pos + 1).unwrap();
      let k_str = format_value(scope, intr, ctx, k, recurse_times, false)?;
      let v_str = format_value(scope, intr, ctx, v, recurse_times, false)?;
      output.push(format!("{k_str} => {v_str}"));
    }
    // Sort to have a halfway reliable output.
    if matches!(ctx.sorted, Sorted::No) {
      output.sort_by(|a, b| {
        let a16: Vec<u16> = a.encode_utf16().collect();
        let b16: Vec<u16> = b.encode_utf16().collect();
        a16.cmp(&b16)
      });
    }
  } else {
    for i in 0..max_length {
      let pos = (i * 2) as u32;
      let k = entries.get_index(scope, pos).unwrap();
      let v = entries.get_index(scope, pos + 1).unwrap();
      let res = vec![
        format_value(scope, intr, ctx, k, recurse_times, false)?,
        format_value(scope, intr, ctx, v, recurse_times, false)?,
      ];
      output.push(reduce_to_single_string(
        scope,
        ctx,
        res,
        "",
        &("[".to_string(), "]".to_string()),
        K_ARRAY_EXTRAS_TYPE,
        recurse_times,
        None,
      )?);
    }
  }
  ctx.indentation_lvl -= 2;
  if remaining > 0 {
    output.push(more_items(remaining as usize));
  }
  Ok(output)
}

fn format_set_iter_inner<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  recurse_times: f64,
  entries: v8::Local<'s, v8::Array>,
  state: u8,
) -> R<Vec<String>> {
  let max_array_length = ctx.max_array_length.max(0.0);
  let total = entries.length() as usize;
  let max_length = (max_array_length as usize).min(total);
  let mut output = Vec::with_capacity(max_length);
  ctx.indentation_lvl += 2;
  for i in 0..max_length {
    let v = entries.get_index(scope, i as u32).unwrap();
    output.push(format_value(scope, intr, ctx, v, recurse_times, false)?);
  }
  ctx.indentation_lvl -= 2;
  if state == K_WEAK && matches!(ctx.sorted, Sorted::No) {
    output.sort_by(|a, b| {
      let a16: Vec<u16> = a.encode_utf16().collect();
      let b16: Vec<u16> = b.encode_utf16().collect();
      a16.cmp(&b16)
    });
  }
  let remaining = total - max_length;
  if remaining > 0 {
    output.push(more_items(remaining));
  }
  Ok(output)
}

fn format_array_buffer<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
) -> R<Vec<String>> {
  let val_len = any_array_buffer_byte_length(scope, value);
  let len = (ctx.max_array_length.max(0.0) as usize).min(val_len);
  let bytes: Option<Vec<u8>> = if let Ok(ab) =
    v8::Local::<v8::ArrayBuffer>::try_from(value)
  {
    if ab.was_detached() {
      None
    } else {
      Some(read_buffer_bytes(ab.data(), len))
    }
  } else if let Ok(sab) = v8::Local::<v8::SharedArrayBuffer>::try_from(value) {
    let store = sab.get_backing_store();
    let data = store.data();
    Some(read_buffer_bytes(data, len))
  } else {
    None
  };
  let Some(bytes) = bytes else {
    return Ok(vec![ctx.stylize(scope, "(detached)", "special")?]);
  };
  let mut hex = String::with_capacity(len * 3);
  for (i, b) in bytes.iter().enumerate() {
    if i > 0 {
      hex.push(' ');
    }
    hex.push_str(&format!("{b:02x}"));
  }
  let mut str_ = hex;
  let remaining = val_len - len;
  if remaining > 0 {
    str_.push_str(&format!(
      " ... {} more byte{}",
      remaining,
      if remaining > 1 { "s" } else { "" }
    ));
  }
  let label = ctx.stylize(scope, "[Uint8Contents]", "special")?;
  Ok(vec![format!("{label}: <{str_}>")])
}

fn read_buffer_bytes(
  data: Option<std::ptr::NonNull<std::ffi::c_void>>,
  len: usize,
) -> Vec<u8> {
  match data {
    Some(ptr) if len > 0 => {
      // SAFETY: the buffer is alive (we hold a Local to it) and len is
      // clamped to its byte length.
      unsafe {
        std::slice::from_raw_parts(ptr.as_ptr() as *const u8, len).to_vec()
      }
    }
    _ => Vec::new(),
  }
}

fn format_namespace_object<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  keys: &mut Vec<v8::Local<'s, v8::Value>>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
) -> R<Vec<String>> {
  let mut output = Vec::with_capacity(keys.len());
  for key in keys.iter() {
    let result = format_property(
      scope,
      intr,
      ctx,
      value,
      recurse_times,
      *key,
      K_OBJECT_TYPE,
      None,
      None,
    );
    match result {
      Ok(s) => output.push(s),
      Err(_) => {
        // Use a stand-in object so indentation and line breaks stay
        // correct, then swap the value for `<uninitialized>`.
        let tmp = v8::Object::new(scope);
        let empty = v8_str(scope, "");
        tmp.set(scope, *key, empty.into());
        let formatted = format_property(
          scope,
          intr,
          ctx,
          tmp.into(),
          recurse_times,
          *key,
          K_OBJECT_TYPE,
          None,
          None,
        )?;
        let pos = formatted.rfind(' ').map(|p| p + 1).unwrap_or(0);
        let uninit = ctx.stylize(scope, "<uninitialized>", "special")?;
        output.push(format!("{}{}", &formatted[..pos], uninit));
      }
    }
  }
  // Reset the keys to an empty array. This prevents duplicated inspection.
  keys.clear();
  Ok(output)
}

// ---------------------------------------------------------------------------
// formatProperty

const COLOR_REGEXP_NOTE: () = ();

/// `removeColors(str)`: strip `\[\d\d?m` sequences.
pub fn remove_colors(s: &str) -> String {
  let _ = COLOR_REGEXP_NOTE;
  let chars: Vec<char> = s.chars().collect();
  let mut out = String::with_capacity(s.len());
  let mut i = 0;
  while i < chars.len() {
    if chars[i] == '\u{1b}'
      && i + 1 < chars.len()
      && chars[i + 1] == '['
      && i + 2 < chars.len()
      && chars[i + 2].is_ascii_digit()
    {
      let mut j = i + 3;
      let mut digits = 1;
      while j < chars.len() && chars[j].is_ascii_digit() && digits < 2 {
        j += 1;
        digits += 1;
      }
      if j < chars.len() && chars[j] == 'm' {
        i = j + 1;
        continue;
      }
    }
    out.push(chars[i]);
    i += 1;
  }
  out
}

#[allow(clippy::too_many_arguments)]
fn format_property<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'_>,
  ctx: &mut Ctx<'s>,
  value: v8::Local<'s, v8::Value>,
  recurse_times: f64,
  key: v8::Local<'s, v8::Value>,
  ty: u8,
  desc_in: Option<v8::Local<'s, v8::Object>>,
  original: Option<v8::Local<'s, v8::Value>>,
) -> R<String> {
  let original = original.unwrap_or(value);
  let value_obj = v8::Local::<v8::Object>::try_from(value)
    .expect("format_property requires an object");
  let mut extra = " ".to_string();

  // desc = desc || ObjectGetOwnPropertyDescriptor(value, key) ||
  //   { value: value[key], enumerable: true }
  let mut desc_value: Option<v8::Local<'s, v8::Value>> = None;
  let mut desc_get: Option<v8::Local<'s, v8::Function>> = None;
  let mut desc_set_present = false;
  let mut enumerable = true;

  let desc_obj = match desc_in {
    Some(d) => Some(d),
    None => {
      // Note: own property descriptor; can throw for proxies, but those are
      // unwrapped before we get here. Errors propagate like in JS.
      v8::tc_scope!(tc, scope);
      match v8::Local::<v8::Name>::try_from(key) {
        Ok(name) => value_obj
          .get_own_property_descriptor(tc, name)
          .and_then(|d| v8::Local::<v8::Object>::try_from(d).ok()),
        Err(_) => {
          // Numeric key (array path): convert to string name.
          let key_str = key.to_rust_string_lossy(tc);
          let name = v8_str(tc, &key_str);
          value_obj
            .get_own_property_descriptor(tc, name.into())
            .and_then(|d| v8::Local::<v8::Object>::try_from(d).ok())
        }
      }
    }
  };

  match desc_obj {
    Some(desc) if !desc.is_undefined() => {
      let v = try_get_str(scope, desc, "value");
      if let Some(v) = v {
        if !v.is_undefined() {
          desc_value = Some(v);
        }
      }
      if desc_value.is_none() {
        if let Some(g) = try_get_str(scope, desc, "get") {
          if g.is_function() {
            desc_get = v8::Local::<v8::Function>::try_from(g).ok();
          }
        }
        if let Some(s) = try_get_str(scope, desc, "set") {
          desc_set_present = s.is_function();
        }
      }
      if let Some(e) = try_get_str(scope, desc, "enumerable") {
        enumerable = e.boolean_value(scope);
      }
    }
    _ => {
      // { value: value[key], enumerable: true }
      let v = js_get(scope, value_obj, key)?;
      if !v.is_undefined() {
        desc_value = Some(v);
      }
      enumerable = true;
    }
  }

  let str_;
  if let Some(v) = desc_value {
    let diff = if !ctx.compact.is_true() || ty != K_OBJECT_TYPE {
      2
    } else {
      3
    };
    ctx.indentation_lvl += diff;
    str_ = format_value(scope, intr, ctx, v, recurse_times, false)?;
    if diff == 3
      && (ctx.break_length as usize) < get_string_width(&str_, ctx.colors)
    {
      extra = format!("\n{}", " ".repeat(ctx.indentation_lvl));
    }
    ctx.indentation_lvl -= diff;
  } else if let Some(getter) = desc_get {
    let label = if desc_set_present {
      "Getter/Setter"
    } else {
      "Getter"
    };
    let should_eval = match ctx.getters {
      Getters::Yes => true,
      Getters::Get => !desc_set_present,
      Getters::Set => desc_set_present,
      Getters::No => false,
    };
    if should_eval {
      let result = js_call(scope, getter, original, &[]);
      match result {
        Ok(tmp) => {
          ctx.indentation_lvl += 2;
          if tmp.is_null() {
            let open = ctx.stylize(scope, &format!("[{label}:"), "special")?;
            let null_s = ctx.stylize(scope, "null", "null")?;
            let close = ctx.stylize(scope, "]", "special")?;
            str_ = format!("{open} {null_s}{close}");
          } else if tmp.is_object() {
            let lab = ctx.stylize(scope, &format!("[{label}]"), "special")?;
            let inner =
              format_value(scope, intr, ctx, tmp, recurse_times, false)?;
            str_ = format!("{lab} {inner}");
          } else {
            let primitive = format_primitive(scope, ctx, tmp, false)?;
            let open = ctx.stylize(scope, &format!("[{label}:"), "special")?;
            let close = ctx.stylize(scope, "]", "special")?;
            str_ = format!("{open} {primitive}{close}");
          }
          ctx.indentation_lvl -= 2;
        }
        Err(err) => {
          let message = {
            let exc = v8::Local::new(scope, &err.0);
            let msg = v8::Local::<v8::Object>::try_from(exc)
              .ok()
              .and_then(|o| try_get_str(scope, o, "message"))
              .map(|m| m.to_rust_string_lossy(scope))
              .unwrap_or_default();
            format!("<Inspection threw ({msg})>")
          };
          let open = ctx.stylize(scope, &format!("[{label}:"), "special")?;
          let close = ctx.stylize(scope, "]", "special")?;
          str_ = format!("{open} {message}{close}");
        }
      }
    } else {
      str_ = ctx.stylize(scope, &format!("[{label}]"), "special")?;
    }
  } else if desc_set_present {
    str_ = ctx.stylize(scope, "[Setter]", "special")?;
  } else {
    str_ = ctx.stylize(scope, "undefined", "undefined")?;
  }

  if ty == K_ARRAY_TYPE {
    return Ok(str_);
  }

  let name;
  if key.is_symbol() {
    let sym = v8::Local::<v8::Symbol>::try_from(key).unwrap();
    let tmp = quote::escape_meta_chars(&symbol_to_string(scope, sym));
    name = ctx.stylize(scope, &tmp, "symbol")?;
  } else {
    let key_str = key.to_rust_string_lossy(scope);
    if quote::is_identifier_like(&key_str) {
      name = if key_str == "__proto__" {
        "['__proto__']".to_string()
      } else {
        ctx.stylize(scope, &key_str, "name")?
      };
    } else {
      let quoted =
        quote::quote_string(&key_str, &ctx.quotes, ctx.escape_sequences);
      name = ctx.stylize(scope, &quoted, "string")?;
    }
  }

  let name = if !enumerable {
    format!("[{name}]")
  } else {
    name
  };
  Ok(format!("{name}:{extra}{str_}"))
}

// ---------------------------------------------------------------------------
// reduceToSingleString and helpers

fn is_below_break_length<'s>(
  ctx: &Ctx<'s>,
  output: &[String],
  start: usize,
  base: &str,
) -> bool {
  // Each entry is separated by at least a comma; start with a total length
  // of at least `output.length`.
  let mut total_length = output.len() + start;
  if (total_length + output.len()) as f64 > ctx.break_length {
    return false;
  }
  for item in output {
    if ctx.colors {
      total_length += utf16_length(&remove_colors(item));
    } else {
      total_length += utf16_length(item);
    }
    if total_length as f64 > ctx.break_length {
      return false;
    }
  }
  // Do not line up properties on the same line if `base` contains line
  // breaks.
  base.is_empty() || !base.contains('\n')
}

#[allow(clippy::too_many_arguments)]
fn reduce_to_single_string<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  ctx: &mut Ctx<'s>,
  mut output: Vec<String>,
  base: &str,
  braces: &(String, String),
  extras_type: u8,
  recurse_times: f64,
  value: Option<v8::Local<'s, v8::Value>>,
) -> R<String> {
  if !ctx.compact.is_true() {
    if let Some(compact_num) = ctx.compact.as_number() {
      if compact_num >= 1.0 {
        // Memorize the original output length.
        let entries = output.len();
        // Group array elements together if the array contains at least six
        // separate entries.
        if extras_type == K_ARRAY_EXTRAS_TYPE && entries > 6 {
          output = group_array_elements(scope, ctx, output, value);
        }
        // Consolidate all entries of the local most inner depth up to
        // `ctx.compact`, as long as the properties are smaller than
        // `ctx.breakLength`.
        if ctx.current_depth - recurse_times < compact_num
          && entries == output.len()
        {
          let start = output.len()
            + ctx.indentation_lvl
            + utf16_length(&braces.0)
            + utf16_length(base)
            + 10;
          if is_below_break_length(ctx, &output, start, base) {
            let joined_output = output.join(", ");
            if !joined_output.contains('\n') {
              let base_part = if !base.is_empty() {
                format!("{base} ")
              } else {
                String::new()
              };
              return Ok(format!(
                "{base_part}{} {joined_output} {}",
                braces.0, braces.1
              ));
            }
          }
        }
      }
    }
    // Line up each entry on an individual line.
    let indentation = format!("\n{}", " ".repeat(ctx.indentation_lvl));
    let base_part = if !base.is_empty() {
      format!("{base} ")
    } else {
      String::new()
    };
    let trailing = if ctx.trailing_comma { "," } else { "" };
    return Ok(format!(
      "{base_part}{}{indentation}  {}{trailing}{indentation}{}",
      braces.0,
      output.join(&format!(",{indentation}  ")),
      braces.1
    ));
  }
  // Line up all entries on a single line in case the entries do not exceed
  // `breakLength`.
  if is_below_break_length(ctx, &output, 0, base) {
    let base_part = if !base.is_empty() {
      format!(" {base}")
    } else {
      String::new()
    };
    return Ok(format!(
      "{}{base_part} {} {}",
      braces.0,
      output.join(", "),
      braces.1
    ));
  }
  let indentation = " ".repeat(ctx.indentation_lvl);
  // If the opening "brace" is too large, like in the case of "Set {", force
  // the first item onto the next line.
  let ln = if base.is_empty() && utf16_length(&braces.0) == 1 {
    " ".to_string()
  } else {
    let base_part = if !base.is_empty() {
      format!(" {base}")
    } else {
      String::new()
    };
    format!("{base_part}\n{indentation}  ")
  };
  Ok(format!(
    "{}{ln}{} {}",
    braces.0,
    output.join(&format!(",\n{indentation}  ")),
    braces.1
  ))
}

/// JS `String.prototype.padStart`/`padEnd` with UTF-16 target lengths.
fn pad_utf16(s: &str, target: usize, pad_start: bool) -> String {
  let len = utf16_length(s);
  if len >= target {
    return s.to_string();
  }
  let padding = " ".repeat(target - len);
  if pad_start {
    format!("{padding}{s}")
  } else {
    format!("{s}{padding}")
  }
}

fn group_array_elements<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  ctx: &mut Ctx<'s>,
  output: Vec<String>,
  value: Option<v8::Local<'s, v8::Value>>,
) -> Vec<String> {
  let mut total_length = 0usize;
  let mut max_length = 0usize;
  let mut output_length = output.len();
  if (ctx.max_array_length as usize) < output.len() {
    // Exclude the "... n more items" part from the calculation.
    output_length -= 1;
  }
  let separator_space = 2; // one space + one separator
  let mut data_len = vec![0usize; output_length];
  for (i, item) in output.iter().take(output_length).enumerate() {
    let len = get_string_width(item, ctx.colors);
    data_len[i] = len;
    total_length += len + separator_space;
    if max_length < len {
      max_length = len;
    }
  }
  // Add two to `maxLength` for the space + comma between two entries.
  let actual_max = max_length + separator_space;
  if ((actual_max * 3 + ctx.indentation_lvl) as f64) < ctx.break_length
    && ((total_length as f64) / (actual_max as f64) > 5.0 || max_length <= 6)
  {
    let approx_char_heights = 2.5f64;
    let average_bias = ((actual_max as f64)
      - (total_length as f64) / (output.len() as f64))
      .sqrt();
    let biased_max = ((actual_max as f64) - 3.0 - average_bias).max(1.0);
    let compact_num = ctx.compact.as_number().unwrap_or(0.0);
    let columns_f =
      ((approx_char_heights * biased_max * (output_length as f64)).sqrt()
        / biased_max)
        .round();
    let columns = columns_f
      .min(
        ((ctx.break_length - ctx.indentation_lvl as f64) / (actual_max as f64))
          .floor(),
      )
      .min(compact_num * 4.0)
      .min(15.0);
    if columns <= 1.0 {
      return output;
    }
    let columns = columns as usize;
    let mut tmp = Vec::new();
    let mut max_line_length = Vec::with_capacity(columns);
    for i in 0..columns {
      let mut line_max_length = 0usize;
      let mut j = i;
      while j < output.len() {
        if data_len.get(j).copied().unwrap_or(0) > line_max_length {
          line_max_length = data_len[j];
        }
        j += columns;
      }
      max_line_length.push(line_max_length + separator_space);
    }
    // Pick alignment: numbers/bigints get right-aligned.
    let mut pad_start_order = true;
    if let Some(value) = value {
      if let Ok(value_obj) = v8::Local::<v8::Object>::try_from(value) {
        for i in 0..output.len() {
          let elem = value_obj.get_index(scope, i as u32);
          let is_numeric = elem
            .map(|e| e.is_number() || e.is_big_int())
            .unwrap_or(false);
          if !is_numeric {
            pad_start_order = false;
            break;
          }
        }
      } else {
        pad_start_order = false;
      }
    }
    let mut i = 0usize;
    while i < output_length {
      let max = (i + columns).min(output_length);
      let mut str_ = String::new();
      let mut j = i;
      while j < max - 1 {
        // Padding compensates for color escape codes (length vs width).
        let padding =
          max_line_length[j - i] + utf16_length(&output[j]) - data_len[j];
        let item = format!("{}, ", output[j]);
        str_.push_str(&pad_utf16(&item, padding, pad_start_order));
        j += 1;
      }
      if pad_start_order {
        let padding = max_line_length[j - i] + utf16_length(&output[j])
          - data_len[j]
          - separator_space;
        str_.push_str(&pad_utf16(&output[j], padding, true));
      } else {
        str_.push_str(&output[j]);
      }
      tmp.push(str_);
      i += columns;
    }
    if (ctx.max_array_length as usize) < output.len() {
      tmp.push(output[output_length].clone());
    }
    return tmp;
  }
  output
}
