// Copyright 2018-2025 the Deno authors. MIT license.
import { assertArrayEquals, assertEquals, test } from "checkin:testing";

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
