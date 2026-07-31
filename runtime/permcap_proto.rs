// Copyright 2018-2026 the Deno authors. MIT license.

//! PROTOTYPE — wayfinder P3 (https://github.com/bartlomieju/wayfinder/issues/28).
//! Grown out of the P1 prototype (issues/19). THROWAWAY CODE. Do not ship, do
//! not extend, delete when the ticket closes.
//!
//! P1 measured a *masking* named property handler in a *debug* build and found
//! ~164 ns/op on every named global read, and could only run at all under
//! `--features hmr` because `create_context` skips `global_template_middleware`
//! when booting from a snapshot. P3 asks what the installation actually costs
//! in a release build, and whether a cheaper installation shape exists.
//!
//! The prior art is `ext/node/global.rs`, removed in denoland/deno#33249 — the
//! same mechanism, shipped in production for ~3 years, boot-from-snapshot
//! included. Two things it did that P1's cut did not:
//!
//!   * `PropertyHandlerFlags::NON_MASKING | HAS_NO_SIDE_EFFECT`, so the handler
//!     only fires for names *absent* from the global object.
//!   * key matching with a UTF-16 length pre-filter + binary search over a
//!     sorted const array — no Rust string conversion per access.
//!
//! ## Variants
//!
//! The interceptor variants are selected at **build time**, not runtime, and
//! that is itself a finding: a named property handler installed via
//! `global_template_middleware` is baked into the startup snapshot, so its
//! presence cannot be gated by a config file or an env var. Only the accessor
//! variant, which installs on the live global after deserialization, can be.
//!
//!   (no feature)                `off` baseline, and the `accessor` variant
//!   --features permcap_mask     masking interceptor (P1's shape)
//!   --features permcap_nonmask  NON_MASKING interceptor + side bag
//!
//! ## Runtime controls (env vars, so no config plumbing)
//!
//!   PERMCAP=1                   enable the callback body / accessor install
//!   PERMCAP_CB=noop|fast|naive  callback body (default `fast`)
//!   PERMCAP_ACCESSOR=1          install per-property accessors on the global
//!   PERMCAP_DENY=a,b            packages denied every guarded global
//!   PERMCAP_TRACE=1             log the install and every guarded read
//!
//! ## Probe surface
//!
//!   globalThis.__permcapWhoami   attribution string at the point of read
//!   Deno.core.ops.op_permcap_probe(bracket)  D2's OpState cell cost

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::MapFnTo;

/// Index in the host-defined-options `PrimitiveArray` where this prototype
/// stores the package id. 0 is taken by `ext/node` (`Boolean(true)` = "is npm")
/// and by `node:vm`'s kind tag; 1 by the vm callback registry key.
pub const PERMCAP_PKG_INDEX: usize = 2;

/// What a read of a guarded global attributes to.
const UNATTRIBUTED: &str = "<unattributed>";

// NOTE(bartlomieju): same thread_local dance as ext/node/global.rs — calling
// `.map_fn_to()` twice on the same function has been observed to yield two
// different pointers, which breaks the external-references table.
thread_local! {
  pub static GETTER_MAP_FN: v8::NamedPropertyGetterCallback = interceptor_getter.map_fn_to();
}

// ---------------------------------------------------------------------------
// Guarded-name matching, ported from ext/node/global.rs
// ---------------------------------------------------------------------------

/// Convert an ASCII string to a UTF-16 byte encoding of the string.
const fn str_to_utf16<const N: usize>(s: &str) -> [u16; N] {
  let mut out = [0_u16; N];
  let mut i = 0;
  let bytes = s.as_bytes();
  assert!(N == bytes.len());
  while i < bytes.len() {
    assert!(bytes[i] < 128, "only works for ASCII strings");
    out[i] = bytes[i] as u16;
    i += 1;
  }
  out
}

/// The authority-bearing globals this prototype guards, plus the whoami probe.
/// THIS LIST MUST BE SORTED by UTF-16 code unit.
#[rustfmt::skip]
const GUARDED: [&[u16]; 7] = [
  &str_to_utf16::<4>("Deno"),
  &str_to_utf16::<9>("WebSocket"),
  &str_to_utf16::<14>("XMLHttpRequest"),
  &str_to_utf16::<15>("__permcapWhoami"),
  &str_to_utf16::<5>("fetch"),
  &str_to_utf16::<9>("navigator"),
  &str_to_utf16::<7>("process"),
];

/// The authority-bearing subset, as Rust strings, for the install path.
const GUARDED_NAMES: [&str; 6] = [
  "Deno",
  "WebSocket",
  "XMLHttpRequest",
  "fetch",
  "navigator",
  "process",
];

const WHOAMI_NAME: &str = "__permcapWhoami";

const GUARDED_INFO: (usize, usize) = {
  let l = GUARDED[0].len();
  let (mut longest, mut shortest, mut i) = (l, l, 1);
  while i < GUARDED.len() {
    let l = GUARDED[i].len();
    if l > longest {
      longest = l
    }
    if l < shortest {
      shortest = l
    }
    i += 1;
  }
  (shortest, longest)
};
const SHORTEST_GUARDED: usize = GUARDED_INFO.0;
const LONGEST_GUARDED: usize = GUARDED_INFO.1;

const WHOAMI: &[u16] = &str_to_utf16::<15>("__permcapWhoami");

/// `Some(true)` for a guarded authority global, `Some(false)` for the whoami
/// probe, `None` for everything else. No heap allocation on any path.
fn classify_key(
  scope: &mut v8::PinScope<'_, '_>,
  key: v8::Local<v8::Name>,
) -> Option<bool> {
  let str: v8::Local<v8::String> = key.try_into().ok()?;
  let len = str.length();
  if !(SHORTEST_GUARDED..=LONGEST_GUARDED).contains(&len) {
    return None;
  }
  let buf = &mut [0u16; LONGEST_GUARDED];
  str.write_v2(scope, 0, buf.as_mut_slice(), v8::WriteFlags::empty());
  let key = &buf[..len];
  GUARDED.binary_search(&key).ok()?;
  Some(key != WHOAMI)
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallbackBody {
  /// Return `kNo` immediately: isolates the *structural* cost of having a
  /// handler on the global object from the cost of the callback body.
  Noop,
  /// UTF-16 key match, attribution only on a guarded hit. The shape a real
  /// implementation would have.
  Fast,
  /// P1's shape: convert every key to a Rust string. Kept for comparability
  /// with P1's debug-build numbers.
  Naive,
}

struct Policy {
  enabled: bool,
  accessors: bool,
  body: CallbackBody,
  trace: bool,
  denied: Vec<String>,
}

fn policy() -> &'static Policy {
  static POLICY: OnceLock<Policy> = OnceLock::new();
  POLICY.get_or_init(|| Policy {
    enabled: std::env::var("PERMCAP").is_ok(),
    accessors: std::env::var("PERMCAP_ACCESSOR").is_ok(),
    body: match std::env::var("PERMCAP_CB").as_deref() {
      Ok("noop") => CallbackBody::Noop,
      Ok("naive") => CallbackBody::Naive,
      _ => CallbackBody::Fast,
    },
    trace: std::env::var("PERMCAP_TRACE").is_ok(),
    denied: std::env::var("PERMCAP_DENY")
      .map(|v| {
        v.split(',')
          .map(str::trim)
          .filter(|s| !s.is_empty())
          .map(str::to_string)
          .collect()
      })
      .unwrap_or_default(),
  })
}

// ---------------------------------------------------------------------------
// Attribution
// ---------------------------------------------------------------------------

/// Read the package id this prototype stamped onto the currently-running
/// script or module. `None` when V8 has no current script, or when the script
/// carries no stamp (first-party code, `ext:` code, separately-compiled code).
///
/// This is the same read `ext/node/global.rs::current_mode` did at index 0.
pub fn current_package(scope: &mut v8::PinScope<'_, '_>) -> Option<String> {
  let data = scope.get_current_host_defined_options()?;
  // SAFETY: host defined options must always be a PrimitiveArray in current V8.
  let arr = unsafe { v8::Local::<v8::PrimitiveArray>::cast_unchecked(data) };
  if arr.length() <= PERMCAP_PKG_INDEX {
    return None;
  }
  let primitive = arr.get(scope, PERMCAP_PKG_INDEX);
  let value: v8::Local<v8::Value> = primitive.into();
  let s = v8::Local::<v8::String>::try_from(value).ok()?;
  Some(s.to_rust_string_lossy(scope))
}

/// The side bag holding the real values of guarded globals, for the variants
/// that remove them from the global object (NON_MASKING and accessors).
struct Bag(v8::Global<v8::Object>);

fn bag_lookup<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  receiver: v8::Local<'s, v8::Object>,
) -> Option<v8::Local<'s, v8::Value>> {
  // NOTE: the slot key is the *inner* type — `set_slot::<T>` takes an `Rc<T>`
  // and `get_slot::<T>` returns `Option<Rc<T>>`. Asking for `Rc<Bag>` here
  // silently returns None, which reads exactly like "the bag is empty".
  let context = scope.get_current_context();
  let bag = context.get_slot::<Bag>()?;
  let bag = v8::Local::new(scope, &bag.0);
  if !bag.has_own_property(scope, key).unwrap_or(false) {
    return None;
  }
  bag.get_with_receiver(scope, key.into(), receiver)
}

/// Shared decision: given a guarded key, either serve it, deny it, or decline.
/// Returns whether the read was handled.
fn decide<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  receiver: v8::Local<'s, v8::Object>,
  rv: &mut v8::ReturnValue<'_, v8::Value>,
  is_authority: bool,
) -> bool {
  if !is_authority {
    let id = current_package(scope).unwrap_or_else(|| UNATTRIBUTED.to_string());
    let s = v8::String::new(scope, &id).unwrap();
    rv.set(s.into());
    return true;
  }

  let pkg = current_package(scope);
  let policy = policy();
  if policy.trace {
    eprintln!(
      "[permcap] guarded read from {}",
      pkg.as_deref().unwrap_or(UNATTRIBUTED)
    );
  }
  if let Some(p) = &pkg {
    if policy.denied.iter().any(|d| d == p) {
      // D3's denial shape: absence is silent, so `typeof fetch` stays
      // "undefined" rather than throwing.
      rv.set_undefined();
      return true;
    }
  }
  // Allowed: serve the real value out of the bag if we took it out of the
  // global, otherwise decline and let the ordinary lookup proceed.
  match bag_lookup(scope, key, receiver) {
    Some(v) => {
      rv.set(v);
      true
    }
    None => false,
  }
}

// ---------------------------------------------------------------------------
// Variant 1 & 2: named property handler on the global object template
// ---------------------------------------------------------------------------

fn interceptor_getter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue<v8::Value>,
) -> v8::Intercepted {
  let policy = policy();
  if !policy.enabled {
    return v8::Intercepted::kNo;
  }
  let is_authority = match policy.body {
    CallbackBody::Noop => return v8::Intercepted::kNo,
    CallbackBody::Naive => {
      // P1's shape, kept only so the release build is comparable to P1's
      // debug numbers. Allocates a Rust String on every named global read.
      let Ok(key_str) = v8::Local::<v8::String>::try_from(key) else {
        return v8::Intercepted::kNo;
      };
      let name = key_str.to_rust_string_lossy(scope);
      match name.as_str() {
        WHOAMI_NAME => false,
        "Deno" | "WebSocket" | "XMLHttpRequest" | "fetch" | "navigator"
        | "process" => true,
        _ => return v8::Intercepted::kNo,
      }
    }
    CallbackBody::Fast => match classify_key(scope, key) {
      Some(is_authority) => is_authority,
      None => return v8::Intercepted::kNo,
    },
  };
  let holder = args.holder();
  if decide(scope, key, holder, &mut rv, is_authority) {
    v8::Intercepted::kYes
  } else {
    v8::Intercepted::kNo
  }
}

#[cfg(any(feature = "permcap_mask", feature = "permcap_nonmask"))]
pub fn permcap_global_template_middleware<'s>(
  _scope: &mut v8::PinScope<'s, '_, ()>,
  template: v8::Local<'s, v8::ObjectTemplate>,
) -> v8::Local<'s, v8::ObjectTemplate> {
  #[allow(unused_mut)]
  let mut config = v8::NamedPropertyHandlerConfiguration::new();

  #[cfg(feature = "permcap_nonmask")]
  {
    // The shape ext/node/global.rs shipped: the handler is only consulted for
    // names that are NOT already present on the global object, which is why
    // the guarded names have to be removed from it (see `install_bag`).
    config = config.flags(
      v8::PropertyHandlerFlags::NON_MASKING
        | v8::PropertyHandlerFlags::HAS_NO_SIDE_EFFECT,
    );
  }

  let config = GETTER_MAP_FN.with(|getter| config.getter_raw(*getter));
  template.set_named_property_handler(config);
  template
}

// ---------------------------------------------------------------------------
// Variant 3: per-property accessors on the live global object
// ---------------------------------------------------------------------------

/// The accessor is a plain `v8::Function` installed through `define_property`
/// with a get/set descriptor, not `Object::set_accessor` — on the *global
/// proxy* the latter silently fails to take, which leaves the name absent
/// altogether and turns every free-name read into a `ReferenceError`.
///
/// The guarded name rides along as the function's `data`, so the callback
/// needs no key match at all: an accessor exists only on names we installed
/// it on, and no unrelated global read can reach this code.
fn accessor_getter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  args: v8::FunctionCallbackArguments<'s>,
  mut rv: v8::ReturnValue<'s, v8::Value>,
) {
  let Ok(key) = v8::Local::<v8::Name>::try_from(args.data()) else {
    return;
  };
  let is_authority = classify_key(scope, key).unwrap_or(true);
  let this = args.this();
  decide(scope, key, this, &mut rv, is_authority);
}

// ---------------------------------------------------------------------------
// Runtime install, called from 99_main.js after bootstrap
// ---------------------------------------------------------------------------

/// Move the guarded globals off the global object into a side bag stored in a
/// context slot, and optionally install accessors in their place.
///
/// This has to happen after bootstrap, not in `global_object_middleware`: the
/// authority-bearing globals are defined by the bootstrap JS, so at
/// context-creation time there is nothing to move. A real implementation would
/// arrange the same thing inside bootstrap, the way `ext/node` populated
/// `__bootstrap.ext_node_nodeGlobals`.
fn install_bag(scope: &mut v8::PinScope<'_, '_>, install_accessors: bool) {
  let context = scope.get_current_context();
  let global = context.global(scope);

  let null = v8::null(scope);
  let bag =
    v8::Object::with_prototype_and_properties(scope, null.into(), &[], &[]);

  // The bag has to be reachable before the first name is removed: moving a
  // guarded global is *observable*. `process` is a lazy accessor on the global
  // (`ext:core/01_core.js`'s `lazyLoad`), so reading it runs `node:process`,
  // which reads free-name `Deno` — which by then we have already deleted.
  // Harvest-then-install as two passes over the whole list deadlocks on that;
  // each name has to be made whole again before the next one is touched.
  let bag_global = v8::Global::new(scope, bag);
  context.set_slot(Rc::new(Bag(bag_global)));

  let undefined = v8::undefined(scope);
  let mut moved: Vec<&str> = Vec::new();
  let mut deferred: Vec<&str> = Vec::new();
  for name in GUARDED_NAMES {
    let key = v8::String::new(scope, name).unwrap();

    // Never *read* a global that is defined lazily: `process` is an accessor
    // installed by `core.createLazyLoader`, so reading it here would force the
    // whole node bootstrap that deno's node-defer work exists to avoid, and
    // would charge every `deno run` for it. A shipped implementation has to
    // wrap the lazy getter rather than call it; this prototype leaves such
    // names unguarded and says so.
    let is_lazy = global
      .get_own_property_descriptor(scope, key.into())
      .and_then(|d| d.to_object(scope))
      .and_then(|d| {
        let get_key = v8::String::new(scope, "get").unwrap();
        d.get(scope, get_key.into())
      })
      .is_some_and(|g| g.is_function());
    if is_lazy {
      deferred.push(name);
      continue;
    }

    let Some(value) = global.get(scope, key.into()) else {
      continue;
    };
    if value.is_undefined() {
      continue;
    }
    bag.set(scope, key.into(), value);
    global.delete(scope, key.into());
    moved.push(name);

    if install_accessors {
      // The point of this variant: installable on the *live* global, after
      // snapshot deserialization, so its presence is a runtime decision. No
      // external reference needed, unlike the interceptor — accessors go on
      // the live global at runtime, so nothing about them is serialized.
      let getter = v8::Function::builder(accessor_getter)
        .data(key.into())
        .build(scope)
        .unwrap();
      let desc = v8::PropertyDescriptor::new_from_get_set(
        getter.into(),
        undefined.into(),
      );
      global.define_property(scope, key.into(), &desc);
    }
  }

  if install_accessors {
    let key = v8::String::new(scope, WHOAMI_NAME).unwrap();
    let getter = v8::Function::builder(accessor_getter)
      .data(key.into())
      .build(scope)
      .unwrap();
    let desc =
      v8::PropertyDescriptor::new_from_get_set(getter.into(), undefined.into());
    global.define_property(scope, key.into(), &desc);
  }

  if policy().trace {
    eprintln!(
      "[permcap] bagged {moved:?} deferred(lazy){deferred:?} accessors={install_accessors}"
    );
  }
}

#[op2(fast)]
fn op_permcap_install(scope: &mut v8::PinScope<'_, '_>) {
  let policy = policy();
  if !policy.enabled {
    return;
  }
  // The NON_MASKING interceptor only fires for absent names, so it needs the
  // bag too; the masking interceptor does not (it sees every read regardless).
  let needs_bag = cfg!(feature = "permcap_nonmask") || policy.accessors;
  if needs_bag {
    install_bag(scope, policy.accessors);
  }
}

// ---------------------------------------------------------------------------
// D2's OpState current-package cell (issues/17)
// ---------------------------------------------------------------------------

/// D2 settled that identity reaches the op layer via a current-package cell in
/// `OpState`, set and restored by a synchronous bracket around each wrapped
/// call. This measures that bracket in isolation: it scales with authority
/// *use*, not with global reads, and it does not touch V8's inline caches.
struct PermcapCell {
  current: RefCell<Option<Rc<str>>>,
  grants: HashMap<Rc<str>, bool>,
  pkg: Rc<str>,
}

impl Default for PermcapCell {
  fn default() -> Self {
    let pkg: Rc<str> = Rc::from("npm:express");
    let mut grants = HashMap::new();
    grants.insert(pkg.clone(), true);
    grants.insert(Rc::from("npm:lodash"), false);
    Self {
      current: RefCell::new(None),
      grants,
      pkg,
    }
  }
}

/// `bracket = false` is the bare op: what a permission check costs today.
/// `bracket = true` adds D2's set/restore plus the grant-table resolution.
#[op2(fast)]
fn op_permcap_probe(state: &mut OpState, bracket: bool) -> u32 {
  if !state.has::<PermcapCell>() {
    state.put(PermcapCell::default());
  }
  let cell = state.borrow::<PermcapCell>();
  if !bracket {
    // Stand-in for today's process-wide check: one map probe, no identity.
    return u32::from(cell.grants.contains_key(&cell.pkg));
  }
  let prev = cell.current.replace(Some(cell.pkg.clone()));
  let allowed = {
    let cur = cell.current.borrow();
    match cur.as_ref() {
      // D2: an unset cell at check time is a hard error.
      None => panic!("permcap: unset current-package cell at check time"),
      Some(p) => cell.grants.get(p).copied().unwrap_or(false),
    }
  };
  *cell.current.borrow_mut() = prev;
  u32::from(allowed)
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

#[cfg(any(feature = "permcap_mask", feature = "permcap_nonmask"))]
deno_core::extension!(
  deno_permcap_proto,
  ops = [op_permcap_install, op_permcap_probe],
  global_template_middleware = permcap_global_template_middleware,
  customizer = |ext: &mut deno_core::Extension| {
    // The interceptor is baked into the startup snapshot, so its callback has
    // to be in the external-references table — the same block ext/node/lib.rs
    // carried for global.rs.
    let external_references = [GETTER_MAP_FN.with(|getter| {
      deno_core::v8::ExternalReference {
        named_getter: *getter,
      }
    })];
    ext.external_references.to_mut().extend(external_references);
  },
);

#[cfg(not(any(feature = "permcap_mask", feature = "permcap_nonmask")))]
deno_core::extension!(
  deno_permcap_proto,
  ops = [op_permcap_install, op_permcap_probe],
);
