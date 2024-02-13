// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  AssertionError,
  assertThrows,
  fail,
} from "./mod.ts";

Deno.test("assertThrows with wrong error class", () => {
  assertThrows(
    () => {
      //This next assertThrows will throw an AssertionError due to the wrong
      //expected error class
      assertThrows(
        () => {
          fail("foo");
        },
        TypeError,
        "Failed assertion: foo",
      );
    },
    AssertionError,
    `Expected error to be instance of "TypeError", but was "AssertionError"`,
  );
});

Deno.test("assertThrows with return type", () => {
  assertThrows(() => {
    throw new Error();
  });
});

Deno.test("assertThrows with non-error value thrown and error class", () => {
  assertThrows(
    () => {
      assertThrows(
        () => {
          throw "Panic!";
        },
        Error,
        "Panic!",
      );
    },
    AssertionError,
    "A non-Error object was thrown.",
  );
});

Deno.test("assertThrows with non-error value thrown", () => {
  assertThrows(
    () => {
      throw "Panic!";
    },
  );
  assertThrows(
    () => {
      throw null;
    },
  );
  assertThrows(
    () => {
      throw undefined;
    },
  );
});

Deno.test("assertThrows with error class", () => {
  assertThrows(
    () => {
      throw new Error("foo");
    },
    Error,
    "foo",
  );
});

Deno.test("assertThrows with thrown error returns caught error", () => {
  const error = assertThrows(
    () => {
      throw new Error("foo");
    },
  );
  assert(error instanceof Error);
  assertEquals(error.message, "foo");
});

Deno.test("assertThrows with thrown non-error returns caught error", () => {
  const stringError = assertThrows(
    () => {
      throw "Panic!";
    },
  );
  assert(typeof stringError === "string");
  assertEquals(stringError, "Panic!");

  const numberError = assertThrows(
    () => {
      throw 1;
    },
  );
  assert(typeof numberError === "number");
  assertEquals(numberError, 1);

  const nullError = assertThrows(
    () => {
      throw null;
    },
  );
  assert(nullError === null);

  const undefinedError = assertThrows(
    () => {
      throw undefined;
    },
  );
  assert(typeof undefinedError === "undefined");
  assertEquals(undefinedError, undefined);
});

Deno.test("Assert Throws Parent Error", () => {
  assertThrows(
    () => {
      throw new AssertionError("Fail!");
    },
    Error,
    "Fail!",
  );
});
