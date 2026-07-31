// Copyright 2018-2026 the Deno authors. MIT license.

//! PROTOTYPE — wayfinder P1 (https://github.com/bartlomieju/wayfinder/issues/19).
//! THROWAWAY CODE. Do not ship, do not extend, delete when the ticket closes.
//!
//! Attacks the mechanism chosen in D3: a named property handler on the global
//! object that attributes the *reading* package via
//! `Isolate::GetCurrentHostDefinedOptions()`, and denies authority-bearing
//! globals to packages with no grant.
//!
//! Only Channel 1 (global accessors) is prototyped. Channel 2 (resolver-mediated
//! imports) is deliberately out of this cut — the question being falsified is
//! whether attribution is reliable at accessor time, which is what everything
//! else rests on.
//!
//! Control (env vars, so no config plumbing):
//!   PERMCAP=1                enable the interceptor at all
//!   PERMCAP_DENY=a,b         packages denied every guarded global
//!   PERMCAP_TRACE=1          log every guarded read to stderr
//!   PERMCAP_GUARD=fetch,...  override the guarded-global list
//!
//! Probe surface for the experiments:
//!   globalThis.__permcapWhoami   -> attribution string at the point of read

use std::collections::HashSet;
use std::sync::OnceLock;

use deno_core::v8;

/// Index in the host-defined-options `PrimitiveArray` where this prototype
/// stores the package id. 0 is taken by `ext/node` (`Boolean(true)` = "is npm")
/// and by `node:vm`'s kind tag; 1 by the vm callback registry key.
pub const PERMCAP_PKG_INDEX: usize = 2;

/// What a read of a guarded global attributes to.
const UNATTRIBUTED: &str = "<unattributed>";

struct Policy {
  enabled: bool,
  noop: bool,
  trace: bool,
  denied: HashSet<String>,
  guarded: HashSet<String>,
}

fn policy() -> &'static Policy {
  static POLICY: OnceLock<Policy> = OnceLock::new();
  POLICY.get_or_init(|| {
    let split = |v: String| -> HashSet<String> {
      v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
    };
    let guarded = std::env::var("PERMCAP_GUARD")
      .map(split)
      .unwrap_or_else(|_| {
        [
          "fetch",
          "Deno",
          "process",
          "WebSocket",
          "XMLHttpRequest",
          "navigator",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
      });
    Policy {
      enabled: std::env::var("PERMCAP").is_ok(),
      noop: std::env::var("PERMCAP_NOOP").is_ok(),
      trace: std::env::var("PERMCAP_TRACE").is_ok(),
      denied: std::env::var("PERMCAP_DENY").map(split).unwrap_or_default(),
      guarded,
    }
  })
}

/// Read the package id this prototype stamped onto the currently-running
/// script or module. `None` when V8 has no current script (host callbacks,
/// microtask drain with an empty stack) or when the script carries no stamp
/// (first-party code, `ext:` code, generated code).
pub fn current_package(scope: &mut v8::PinScope<'_, '_>) -> Option<String> {
  let data = scope.get_current_host_defined_options()?;
  // SAFETY: same cast deno_core does in `read_host_defined_options_kind` —
  // V8 guarantees host-defined options are a PrimitiveArray, and rusty_v8
  // has no checked conversion.
  let arr: v8::Local<v8::PrimitiveArray> = unsafe {
    std::mem::transmute::<v8::Local<v8::Data>, v8::Local<v8::PrimitiveArray>>(
      data,
    )
  };
  if arr.length() <= PERMCAP_PKG_INDEX {
    return None;
  }
  let primitive = arr.get(scope, PERMCAP_PKG_INDEX);
  let value: v8::Local<v8::Value> = primitive.into();
  let s = v8::Local::<v8::String>::try_from(value).ok()?;
  Some(s.to_rust_string_lossy(scope))
}

fn permcap_getter(
  scope: &mut v8::PinScope,
  key: v8::Local<v8::Name>,
  _args: v8::PropertyCallbackArguments,
  mut rv: v8::ReturnValue<v8::Value>,
) -> v8::Intercepted {
  let policy = policy();
  if !policy.enabled {
    return v8::Intercepted::kNo;
  }
  // PERMCAP_NOOP isolates the *structural* cost of having a named property
  // handler on the global object (V8 gives up its global-load inline caches)
  // from the cost of this prototype's deliberately naive callback body.
  if policy.noop {
    return v8::Intercepted::kNo;
  }
  let Ok(key_str) = v8::Local::<v8::String>::try_from(key) else {
    return v8::Intercepted::kNo;
  };
  // NOTE(prototype): converting every global-property name to a Rust string is
  // the naive thing and is part of what the cost experiment measures. A real
  // implementation would compare against cached `v8::Global<v8::String>`s or
  // key off an internal field.
  let name = key_str.to_rust_string_lossy(scope);

  if name == "__permcapWhoami" {
    let id = current_package(scope).unwrap_or_else(|| UNATTRIBUTED.to_string());
    let s = v8::String::new(scope, &id).unwrap();
    rv.set(s.into());
    return v8::Intercepted::kYes;
  }

  if !policy.guarded.contains(&name) {
    return v8::Intercepted::kNo;
  }

  let pkg = current_package(scope);
  if policy.trace {
    eprintln!(
      "[permcap] read {name} from {}",
      pkg.as_deref().unwrap_or(UNATTRIBUTED)
    );
  }
  match &pkg {
    Some(p) if policy.denied.contains(p) => {
      // D3's denial shape: absence is silent, so `typeof fetch` stays
      // "undefined" rather than throwing.
      rv.set_undefined();
      v8::Intercepted::kYes
    }
    _ => v8::Intercepted::kNo,
  }
}

pub fn permcap_global_template_middleware<'s>(
  _scope: &mut v8::PinScope<'s, '_, ()>,
  template: v8::Local<'s, v8::ObjectTemplate>,
) -> v8::Local<'s, v8::ObjectTemplate> {
  // The interceptor is only installed when PERMCAP is set, so the same binary
  // gives a true "feature off" baseline for the cost experiment: with the env
  // var unset the global object has no named property handler at all.
  if policy().enabled {
    template.set_named_property_handler(
      v8::NamedPropertyHandlerConfiguration::new().getter(permcap_getter),
    );
  }
  template
}

deno_core::extension!(
  deno_permcap_proto,
  global_template_middleware = permcap_global_template_middleware,
);
