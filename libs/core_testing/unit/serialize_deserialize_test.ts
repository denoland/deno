// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assert,
  assertArrayEquals,
  assertEquals,
  assertThrows,
  test,
} from "checkin:testing";

test(function testIssue20727() {
  // https://github.com/denoland/deno/issues/20727
  const ab = new ArrayBuffer(10);
  const transferList = [ab];
  Deno.core.serialize(
    { ab },
    { transferredArrayBuffers: transferList },
  );

  // The shared_array_buffer_store replaces this with a number
  assertEquals(typeof transferList[0], "number");
});

test(function testIssue20727b() {
  const data = {
    array1: new Uint32Array([]),
    array2: new Float32Array([]),
  };
  const transferList = [
    data.array1.buffer,
    data.array2.buffer,
  ];
  const serializedMultipleTransferredBuffers = Deno.core.serialize(
    { id: 2, data },
    { transferredArrayBuffers: transferList },
  );

  // The shared_array_buffer_store replaces these with a number
  assertEquals(typeof transferList[0], "number");
  assertEquals(typeof transferList[1], "number");

  // should not throw
  Deno.core.deserialize(
    serializedMultipleTransferredBuffers,
    { transferredArrayBuffers: transferList },
  );
});

test(function testEmptyString() {
  const emptyString = "";
  const emptyStringSerialized = [255, 15, 34, 0];
  assertArrayEquals(
    Deno.core.serialize(emptyString),
    emptyStringSerialized,
  );
  assertEquals(
    Deno.core.deserialize(
      new Uint8Array(emptyStringSerialized),
    ),
    emptyString,
  );
});

test(function testPrimitiveArray() {
  const primitiveValueArray = ["test", "a", null, undefined];
  // deno-fmt-ignore
  const primitiveValueArraySerialized = [
    255, 15, 65, 4, 34, 4, 116, 101, 115, 116,
    34, 1, 97, 48, 95, 36, 0, 4,
  ];
  assertArrayEquals(
    Deno.core.serialize(primitiveValueArray),
    primitiveValueArraySerialized,
  );
  assertArrayEquals(
    Deno.core.deserialize(
      new Uint8Array(primitiveValueArraySerialized),
    ),
    primitiveValueArray,
  );
});

test(function testCircularObject() {
  const circularObject = { test: null as unknown, test2: "dd", test3: "aa" };
  circularObject.test = circularObject;
  // deno-fmt-ignore
  const circularObjectSerialized = [
    255, 15, 111, 34, 4, 116, 101, 115,
    116, 94, 0, 34, 5, 116, 101, 115,
    116, 50, 34, 2, 100, 100, 34, 5,
    116, 101, 115, 116, 51, 34, 2, 97,
    97, 123, 3,
  ];
  assertArrayEquals(
    Deno.core.serialize(circularObject),
    circularObjectSerialized,
  );

  const deserializedCircularObject = Deno.core.deserialize(
    new Uint8Array(circularObjectSerialized),
  );
  assertEquals(deserializedCircularObject.test, deserializedCircularObject);
});

test(function structuredClone() {
  const primitiveValueArray = ["test", "a", null, undefined];
  assertArrayEquals(
    Deno.core.structuredClone(primitiveValueArray),
    primitiveValueArray,
  );

  const circularObject = { test: null as unknown, test2: "dd", test3: "aa" };
  circularObject.test = circularObject;
  const cloned = Deno.core.structuredClone(circularObject);
  assertEquals(cloned.test, cloned);
  assertEquals(cloned.test2, circularObject.test2);
  assertEquals(cloned.test3, circularObject.test3);
});

test(function cloneableResourceStructuredClone() {
  // Create a class that supports structured cloning via hostObjectBrand
  class MyCloneable {
    value: string;
    constructor(value: string) {
      this.value = value;
      // deno-lint-ignore no-this-alias
      const self = this;
      this[Deno.core.hostObjectBrand] = () => ({
        type: "MyCloneable",
        value: self.value,
      });
    }
  }

  Deno.core.registerCloneableResource(
    "MyCloneable",
    (data: { value: string }) => new MyCloneable(data.value),
  );

  const original = new MyCloneable("hello");
  const cloned = Deno.core.structuredClone(original);

  assert(cloned instanceof MyCloneable);
  assertEquals(cloned.value, "hello");
  assert(cloned !== original);
});

test(function cloneableResourceSerializeDeserialize() {
  // Use a different name to avoid duplicate registration
  class AnotherCloneable {
    data: number;
    constructor(data: number) {
      this.data = data;
      // deno-lint-ignore no-this-alias
      const self = this;
      this[Deno.core.hostObjectBrand] = () => ({
        type: "AnotherCloneable",
        data: self.data,
      });
    }
  }

  Deno.core.registerCloneableResource(
    "AnotherCloneable",
    (d: { data: number }) => new AnotherCloneable(d.data),
  );

  const original = new AnotherCloneable(42);
  const serialized = Deno.core.serialize(original);
  const deserialized = Deno.core.deserialize(serialized, {
    deserializers: Deno.core.getCloneableDeserializers(),
  });

  assert(deserialized instanceof AnotherCloneable);
  assertEquals(deserialized.data, 42);
});

test(function cloneableResourceNestedInObject() {
  class NestedCloneable {
    name: string;
    constructor(name: string) {
      this.name = name;
      // deno-lint-ignore no-this-alias
      const self = this;
      this[Deno.core.hostObjectBrand] = () => ({
        type: "NestedCloneable",
        name: self.name,
      });
    }
  }

  Deno.core.registerCloneableResource(
    "NestedCloneable",
    (d: { name: string }) => new NestedCloneable(d.name),
  );

  const obj = {
    foo: "bar",
    nested: new NestedCloneable("test"),
    num: 123,
  };

  const cloned = Deno.core.structuredClone(obj);

  assertEquals(cloned.foo, "bar");
  assertEquals(cloned.num, 123);
  assert(cloned.nested instanceof NestedCloneable);
  assertEquals(cloned.nested.name, "test");
});

test(function cloneableResourceDuplicateRegistrationThrows() {
  Deno.core.registerCloneableResource("DuplicateTest", () => {});
  assertThrows(
    () => Deno.core.registerCloneableResource("DuplicateTest", () => {}),
    Error,
    "already registered",
  );
});
