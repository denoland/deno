// Copyright 2018-2025 the Deno authors. MIT license.

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
