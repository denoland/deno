// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertThrows,
  loadTestLibrary,
} from "./common.js";

const object = loadTestLibrary();

Deno.test("napi object", function () {
  const r = object.test_object_new(1, "hello");
  assertEquals(typeof r, "object");
  assertEquals(r[0], 1);
  assertEquals(r[1], "hello");

  const r1 = object.test_object_get(r);
  assert(r === r1);

  const r2 = object.test_object_attr_property(r);
  assert(r === r2);
  assertThrows(
    () => {
      r2.self = "2";
    },
    Error,
    "Cannot assign to read only property 'self' of object '#<Object>'",
  );

  assertThrows(
    () => {
      r2.method = () => {};
    },
    Error,
    "Cannot assign to read only property 'method' of object '#<Object>'",
  );
});

Deno.test("napi create_object_with_properties", function () {
  const objectWithProperties = object.test_create_object_with_properties();
  assertEquals(typeof objectWithProperties, "object");
  assertEquals(objectWithProperties.name, "Foo");
  assertEquals(objectWithProperties.age, 42);
  assertEquals(objectWithProperties.active, true);
});

Deno.test("napi create_object_with_properties_empty", function () {
  const emptyObject = object.test_create_object_with_properties_empty();
  assertEquals(typeof emptyObject, "object");
  assertEquals(Object.keys(emptyObject).length, 0);
});

Deno.test("napi create_object_with_custom_prototype", function () {
  const objectWithCustomPrototype = object
    .test_create_object_with_custom_prototype();
  assertEquals(typeof objectWithCustomPrototype, "object");
  assertEquals(Object.getOwnPropertyNames(objectWithCustomPrototype).length, 1);
  assertEquals(
    Object.getOwnPropertyNames(objectWithCustomPrototype)[0],
    "value",
  );
  assertEquals(objectWithCustomPrototype.value, 42);
  assertEquals(typeof objectWithCustomPrototype.test, "function");
});

Deno.test("napi create_object_with_named_properties", function () {
  const obj = object.test_create_object_with_named_properties();
  assertEquals(typeof obj, "object");
  assertEquals(obj.name, "Foo");
  assertEquals(obj.age, 42);
  assertEquals(obj.active, true);
});

Deno.test("napi create_object_with_named_properties_empty", function () {
  const emptyObj = object.test_create_object_with_named_properties_empty();
  assertEquals(typeof emptyObj, "object");
  assertEquals(Object.keys(emptyObj).length, 0);
});

Deno.test("napi get_property_names", function () {
  const obj = { a: 1, b: 2, c: 3 };
  const names = object.test_get_property_names(obj);
  assertEquals(Array.from(names).sort(), ["a", "b", "c"]);
});

Deno.test("napi has_property", function () {
  const obj = { foo: 1 };
  assertEquals(object.test_has_property(obj, "foo"), true);
  assertEquals(object.test_has_property(obj, "bar"), false);
  // Inherited property
  assertEquals(object.test_has_property(obj, "toString"), true);
});

Deno.test("napi has_own_property", function () {
  const obj = { foo: 1 };
  assertEquals(object.test_has_own_property(obj, "foo"), true);
  assertEquals(object.test_has_own_property(obj, "bar"), false);
  // Inherited property should return false for has_own
  assertEquals(object.test_has_own_property(obj, "toString"), false);
});

Deno.test("napi delete_property", function () {
  const obj = { foo: 1, bar: 2 };
  assertEquals(object.test_delete_property(obj, "foo"), true);
  assertEquals(obj.foo, undefined);
  assertEquals(obj.bar, 2);
});

Deno.test("napi has_named_property", function () {
  const obj = { hello: "world" };
  assertEquals(object.test_has_named_property(obj, "hello"), true);
  assertEquals(object.test_has_named_property(obj, "missing"), false);
});

Deno.test("napi has_element / delete_element", function () {
  const arr = [10, 20, 30];
  assertEquals(object.test_has_element(arr, 0), true);
  assertEquals(object.test_has_element(arr, 2), true);
  assertEquals(object.test_has_element(arr, 5), false);

  assertEquals(object.test_delete_element(arr, 1), true);
  assertEquals(arr[1], undefined);
  assertEquals(arr.length, 3); // length stays, element becomes sparse
});

Deno.test("napi object_freeze", function () {
  const obj = { x: 1 };
  const frozen = object.test_object_freeze(obj);
  assert(Object.isFrozen(frozen));
  assertThrows(() => {
    frozen.x = 2;
  });
});

Deno.test("napi object_seal", function () {
  const obj = { x: 1 };
  const sealed = object.test_object_seal(obj);
  assert(Object.isSealed(sealed));
  // Can modify existing properties
  sealed.x = 2;
  assertEquals(sealed.x, 2);
  // Cannot add new properties
  assertThrows(() => {
    sealed.y = 3;
  });
});

Deno.test("napi get_prototype", function () {
  const proto = { greet() {} };
  const obj = Object.create(proto);
  const result = object.test_get_prototype(obj);
  assert(result === proto);
});

Deno.test("napi strict_equals", function () {
  const a = {};
  assertEquals(object.test_strict_equals(a, a), true);
  assertEquals(object.test_strict_equals(a, {}), false);
  assertEquals(object.test_strict_equals(1, 1), true);
  assertEquals(object.test_strict_equals(1, "1"), false);
  assertEquals(object.test_strict_equals(null, null), true);
  assertEquals(object.test_strict_equals(null, undefined), false);
});

Deno.test("napi get_all_property_names", function () {
  const proto = { inherited: true };
  const obj = Object.create(proto);
  obj.own = true;
  const names = object.test_get_all_property_names(obj);
  const arr = Array.from(names);
  assert(arr.includes("own"));
  assert(arr.includes("inherited"));
});
