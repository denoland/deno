// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function assertArrayEquals(a1, a2) {
  if (a1.length !== a2.length) throw Error("assert");

  for (const index in a1) {
    if (a1[index] !== a2[index]) {
      throw Error("assert");
    }
  }
}

function main() {
  const emptyString = "";
  const emptyStringSerialized = [34, 0];
  assertArrayEquals(Deno.core.serialize(emptyString), emptyStringSerialized);
  assert(
    Deno.core.deserialize(new Uint8Array(emptyStringSerialized)) ===
      emptyString,
  );

  const primitiveValueArray = ["test", "a", null, undefined];
  // deno-fmt-ignore
  const primitiveValueArraySerialized = [
    65, 4, 34, 4, 116, 101, 115, 116,
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

  const circularObject = { test: null, test2: "dd", test3: "aa" };
  circularObject.test = circularObject;
  // deno-fmt-ignore
  const circularObjectSerialized = [
    111, 34, 4, 116, 101, 115, 116, 94,
    0, 34, 5, 116, 101, 115, 116, 50,
    34, 2, 100, 100, 34, 5, 116, 101,
    115, 116, 51, 34, 2, 97, 97, 123,
    3,
  ];

  assertArrayEquals(
    Deno.core.serialize(circularObject),
    circularObjectSerialized,
  );

  const deserializedCircularObject = Deno.core.deserialize(
    new Uint8Array(circularObjectSerialized),
  );
  assert(deserializedCircularObject.test == deserializedCircularObject);
}

main();
