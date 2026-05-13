// Copyright 2018-2026 the Deno authors. MIT license.
//
// Semantic round-trips through the compat layer against the mock backend.
//
// `tests/refcount.rs` verifies that the GC discipline is correct under
// every scope-nesting pattern. This file verifies that the actual *value*
// surface — property get/set, promise state transitions, TryCatch capture,
// bytecode-cache round-trip — observably works against the mock arena.
//
// Real-engine equivalents live in `tests/real_engine.rs` and run only with
// `--features link_quickjs`. Mock-specific assumptions encoded here (e.g.
// promise state immediately after resolver invocation) don't always hold
// against the real engine — that's expected, the two backends are
// separately validated.

#![cfg(not(feature = "link_quickjs"))]

use qjs_v8_compat::v8;

fn fresh() -> v8::OwnedIsolate {
  v8::OwnedIsolate::new(v8::CreateParams::default())
}

#[test]
fn object_string_property_round_trip() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let obj = v8::Local::<v8::Object>::new(&mut scope);
    let value = v8::Local::<v8::String>::new(&mut scope, "hello").unwrap();
    let value_v: v8::Local<v8::Value> = unsafe {
      std::mem::transmute::<v8::Local<v8::String>, v8::Local<v8::Value>>(value)
    };
    assert!(obj.set_str(&mut scope, "greeting", value_v));

    let got = obj.get_str(&mut scope, "greeting").unwrap();
    assert!(got.is_string());
    assert_eq!(got.to_rust_string_lossy(&mut scope), "hello");
  }
  drop(iso);
}

#[test]
fn object_indexed_property_round_trip() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let arr = v8::Local::<v8::Array>::new(&mut scope, 0);
    let elem = v8::Local::<v8::Object>::new(&mut scope);
    let elem_v: v8::Local<v8::Value> = unsafe {
      std::mem::transmute::<v8::Local<v8::Object>, v8::Local<v8::Value>>(elem)
    };
    // Reinterpret Array as Object for indexed access.
    let as_obj: v8::Local<v8::Object> = unsafe {
      std::mem::transmute::<v8::Local<v8::Array>, v8::Local<v8::Object>>(arr)
    };
    assert!(as_obj.set_index(&mut scope, 0, elem_v));
    let got = as_obj.get_index(&mut scope, 0).unwrap();
    assert!(got.is_object());
  }
  drop(iso);
}

#[test]
fn object_delete_property() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let obj = v8::Local::<v8::Object>::new(&mut scope);
    let value = v8::Local::<v8::String>::new(&mut scope, "x").unwrap();
    let value_v: v8::Local<v8::Value> = unsafe {
      std::mem::transmute::<v8::Local<v8::String>, v8::Local<v8::Value>>(value)
    };
    obj.set_str(&mut scope, "k", value_v);
    let key = v8::Local::<v8::String>::new(&mut scope, "k").unwrap();
    let key_v: v8::Local<v8::Value> = unsafe {
      std::mem::transmute::<v8::Local<v8::String>, v8::Local<v8::Value>>(key)
    };
    assert_eq!(obj.has(&mut scope, key_v), Some(true));
    assert_eq!(obj.delete(&mut scope, key_v), Some(true));
    assert_eq!(obj.has(&mut scope, key_v), Some(false));
  }
  drop(iso);
}

#[test]
fn promise_resolves_to_fulfilled() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let resolver = v8::Local::<v8::PromiseResolver>::new(&mut scope).unwrap();
    let promise = resolver.get_promise(&mut scope);
    assert_eq!(promise.state_with(&mut scope), v8::PromiseState::Pending);

    let val = v8::Local::<v8::String>::new(&mut scope, "ok").unwrap();
    let val_v: v8::Local<v8::Value> = unsafe {
      std::mem::transmute::<v8::Local<v8::String>, v8::Local<v8::Value>>(val)
    };
    assert_eq!(resolver.resolve(&mut scope, val_v), Some(true));
    assert_eq!(promise.state_with(&mut scope), v8::PromiseState::Fulfilled);

    let result = promise.result(&mut scope);
    assert_eq!(result.to_rust_string_lossy(&mut scope), "ok");
  }
  qjs_v8_compat::v8::_clear_resolving_funcs_for_tests();
  drop(iso);
}

#[test]
fn promise_rejects_to_rejected() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let resolver = v8::Local::<v8::PromiseResolver>::new(&mut scope).unwrap();
    let promise = resolver.get_promise(&mut scope);
    let val = v8::Local::<v8::String>::new(&mut scope, "no").unwrap();
    let val_v: v8::Local<v8::Value> = unsafe {
      std::mem::transmute::<v8::Local<v8::String>, v8::Local<v8::Value>>(val)
    };
    assert_eq!(resolver.reject(&mut scope, val_v), Some(true));
    assert_eq!(promise.state_with(&mut scope), v8::PromiseState::Rejected);
  }
  qjs_v8_compat::v8::_clear_resolving_funcs_for_tests();
  drop(iso);
}

#[test]
fn trycatch_captures_thrown_exception() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let msg = v8::Local::<v8::String>::new(&mut scope, "bad").unwrap();
    let err = v8::Exception::type_error(&mut scope, msg);
    // err carries `name` and `message` as own properties — verify before throw.
    let err_obj: v8::Local<v8::Object> = unsafe {
      std::mem::transmute::<v8::Local<v8::Value>, v8::Local<v8::Object>>(err)
    };
    let got_name = err_obj.get_str(&mut scope, "name").unwrap();
    assert_eq!(got_name.to_rust_string_lossy(&mut scope), "TypeError");

    // Throw, then catch.
    scope.isolate().throw_exception(err);
    let mut tc = v8::TryCatch::new(&mut scope);
    assert!(tc.has_caught());
    let caught = tc.exception().unwrap();
    assert!(caught.is_object());
  }
  drop(iso);
}

#[test]
fn trycatch_without_throw_has_no_exception() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let mut tc = v8::TryCatch::new(&mut scope);
    assert!(!tc.has_caught());
    assert!(tc.exception().is_none());
  }
  drop(iso);
}

#[test]
fn trycatch_rethrow_restores_pending() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let msg = v8::Local::<v8::String>::new(&mut scope, "x").unwrap();
    let err = v8::Exception::error(&mut scope, msg);
    scope.isolate().throw_exception(err);
    {
      let mut tc = v8::TryCatch::new(&mut scope);
      assert!(tc.has_caught());
      let _ = tc.rethrow();
    }
    // After the inner TryCatch drops with rethrow, the outer scope sees a
    // pending exception again.
    let mut tc2 = v8::TryCatch::new(&mut scope);
    assert!(tc2.has_caught());
    let _exc = tc2.exception();
  }
  drop(iso);
}

#[test]
fn snapshot_blob_empty_round_trips() {
  let creator = v8::SnapshotCreator::new(None);
  let blob = creator.create_blob(v8::FunctionCodeHandling::Keep).unwrap();
  // Empty blob is sentinel-empty (no header); entries() returns Some(empty).
  assert!(blob.entries().unwrap().is_empty());
}

#[test]
fn snapshot_blob_carries_added_source() {
  let mut iso = fresh();
  let mut creator = v8::SnapshotCreator::new(None);
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    // The mock backend's eval() returns `undefined` — strings can't be
    // compiled to bytecode under the mock. Instead, hand it a refcounted
    // value we know we can serialize: a string.
    let value = v8::Local::<v8::String>::new(&mut scope, "hello").unwrap();
    let blob_v = qjs_v8_compat::sys::write_bytecode(
      scope.ctx_for_test(),
      value.raw_for_test(),
    )
    .unwrap();
    // Sidecar the bytecode into the creator manually to exercise the blob
    // format. (`add_source` itself depends on a working eval, which the
    // mock backend does not provide.)
    creator.push_entry_for_test("file:///main.js".to_string(), blob_v);
  }
  let blob = creator.create_blob(v8::FunctionCodeHandling::Keep).unwrap();
  let entries = blob.entries().unwrap();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].0, "file:///main.js");

  // Round-trip: read the blob back via `load_blob_entries`.
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let restored = v8::load_blob_entries(&mut scope, &blob).unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].0, "file:///main.js");
    assert!(restored[0].1.is_string());
    assert_eq!(restored[0].1.to_rust_string_lossy(&mut scope), "hello");
  }
  drop(iso);
}

#[test]
fn bytecode_round_trip_preserves_nested_objects() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let outer = v8::Local::<v8::Object>::new(&mut scope);
    let inner_str = v8::Local::<v8::String>::new(&mut scope, "v").unwrap();
    let inner_str_v: v8::Local<v8::Value> = unsafe {
      std::mem::transmute::<v8::Local<v8::String>, v8::Local<v8::Value>>(
        inner_str,
      )
    };
    outer.set_str(&mut scope, "k", inner_str_v);

    let bc = qjs_v8_compat::sys::write_bytecode(
      scope.ctx_for_test(),
      outer.raw_for_test(),
    )
    .unwrap();
    let restored_raw =
      qjs_v8_compat::sys::read_bytecode(scope.ctx_for_test(), &bc);
    assert!(qjs_v8_compat::sys::jsv_is_object(&restored_raw));
    scope.track_owned_for_test(restored_raw);
    let restored: v8::Local<v8::Object> = unsafe {
      std::mem::transmute(v8::Local::<v8::Value>::from_raw_for_test(
        restored_raw,
      ))
    };
    let got = restored.get_str(&mut scope, "k").unwrap();
    assert!(got.is_string());
    assert_eq!(got.to_rust_string_lossy(&mut scope), "v");
  }
  drop(iso);
}
