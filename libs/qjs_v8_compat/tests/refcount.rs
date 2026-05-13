// Copyright 2018-2026 the Deno authors. MIT license.
//
// Refcount-balance tests against the mock backend.
//
// These run *without* QuickJS linked. They exercise the central invariant
// of the compat layer: for every nesting of HandleScope/EscapableHandle/
// Global/etc, the arena must end up empty when the OwnedIsolate is dropped.

use qjs_v8_compat::v8;

fn fresh() -> v8::OwnedIsolate {
  v8::OwnedIsolate::new(v8::CreateParams::default())
}

#[test]
fn empty_isolate_drops_clean() {
  let iso = fresh();
  drop(iso);
}

#[test]
fn single_scope_with_object() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let _obj = v8::Local::<v8::Object>::new(&mut scope);
    // Scope drops here; object is freed.
  }
  // Isolate drops here; arena must be empty.
  drop(iso);
}

#[test]
fn nested_scopes_each_drop_their_handles() {
  let mut iso = fresh();
  let mut outer = v8::HandleScope::new(&mut iso);
  assert_eq!(outer.owned_count(), 0);
  let _outer_obj = v8::Local::<v8::Object>::new(&mut outer);
  assert_eq!(outer.owned_count(), 1);
  {
    // Inner scope: alloc, then drop. Outer count unchanged.
    let mut inner = v8::HandleScope::new(unsafe {
      &mut *(outer.isolate() as *mut v8::Isolate as *mut v8::OwnedIsolate)
    });
    let _inner_obj = v8::Local::<v8::Object>::new(&mut inner);
    let _inner_str = v8::Local::<v8::String>::new(&mut inner, "hi").unwrap();
    assert_eq!(inner.owned_count(), 2);
  }
  // Inner dropped; outer still has its one obj.
  assert_eq!(outer.owned_count(), 1);
  drop(outer);
  drop(iso);
}

#[test]
fn global_extends_lifetime_past_scope() {
  let mut iso = fresh();
  let global = {
    let mut scope = v8::HandleScope::new(&mut iso);
    let obj = v8::Local::<v8::Object>::new(&mut scope);
    v8::Global::new(&mut scope, obj)
    // scope drops; the scope-bound refcount goes away, but the Global's
    // separate refcount keeps the entry alive in the arena.
  };
  drop(global);
  // Now the arena should be empty.
  drop(iso);
}

#[test]
fn global_to_local_round_trip() {
  let mut iso = fresh();
  let global = {
    let mut scope = v8::HandleScope::new(&mut iso);
    let obj = v8::Local::<v8::Object>::new(&mut scope);
    v8::Global::new(&mut scope, obj)
  };
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    let _l = global.to_local(&mut scope);
    // Local goes away with scope.
  }
  drop(global);
  drop(iso);
}

#[test]
fn primitives_have_no_refcount() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    // None of these are refcounted; the arena stays empty.
    let _undef = v8::undefined(&mut scope);
    let _null = v8::null(&mut scope);
    let _int = v8::Local::<v8::Integer>::new(&mut scope, 42);
    let _num = v8::Local::<v8::Number>::new(&mut scope, 3.14);
    let _b = v8::Local::<v8::Boolean>::new(&mut scope, true);
    // After dropping scope and isolate the arena should remain empty —
    // the only refcounted entries would be JSStrings etc.
  }
  drop(iso);
}

#[test]
fn many_strings_in_one_scope() {
  let mut iso = fresh();
  {
    let mut scope = v8::HandleScope::new(&mut iso);
    for i in 0..1000 {
      let _s =
        v8::Local::<v8::String>::new(&mut scope, &format!("s{}", i)).unwrap();
    }
    // All freed when scope drops.
  }
  drop(iso);
}

#[test]
fn refcounted_value_types_detected_as_objects() {
  let mut iso = fresh();
  let mut scope = v8::HandleScope::new(&mut iso);
  let obj = v8::Local::<v8::Object>::new(&mut scope);
  let as_value: v8::Local<v8::Value> = unsafe {
    // Force the upcast — our shim doesn't have an Object→Value impl yet.
    std::mem::transmute::<v8::Local<v8::Object>, v8::Local<v8::Value>>(obj)
  };
  assert!(as_value.is_object());
  assert!(!as_value.is_string());
  assert!(!as_value.is_undefined());
}

#[test]
fn type_discrimination_on_primitives() {
  let mut iso = fresh();
  let mut scope = v8::HandleScope::new(&mut iso);
  let undef = v8::undefined(&mut scope);
  let undef_val: v8::Local<v8::Value> = undef.into();
  assert!(undef_val.is_undefined());
  assert!(!undef_val.is_null());

  let null = v8::null(&mut scope);
  let null_val: v8::Local<v8::Value> = null.into();
  assert!(null_val.is_null());
  assert!(!null_val.is_undefined());
  assert!(undef_val.is_null_or_undefined());
  assert!(null_val.is_null_or_undefined());
}
