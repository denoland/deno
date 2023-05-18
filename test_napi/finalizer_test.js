// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows, loadTestLibrary } from "./common.js";

const finalizer = loadTestLibrary();

let finalized = {};
let callCount = 0;
const callback = () => {
  callCount++;
};

finalizer.addFinalizerOnly(finalized, callback);
finalizer.addFinalizerOnly(finalized, callback);

Deno.test("finalizers", async function () {
  // Ensure attached items cannot be retrieved.
  assertThrows(() => finalizer.unwrap(finalized), Error, "Invalid argument");

  // Ensure attached items cannot be removed.
  assertThrows(
    () => finalizer.removeWrap(finalized),
    Error,
    "Invalid argument",
  );
  finalized = null;
  gc();

  // Add an item to an object that is already wrapped, and ensure that its
  // finalizer as well as the wrap finalizer gets called.
  async function testFinalizeAndWrap() {
    assertEquals(finalizer.derefItemWasCalled(), false);
    let finalizeAndWrap = {};
    finalizer.wrap(finalizeAndWrap);
    finalizer.addFinalizerOnly(finalizeAndWrap, common.mustCall());
    finalizeAndWrap = null;
    // TODO:
    // await common.gcUntil(
    //   "test finalize and wrap",
    //   () => test_general.derefItemWasCalled(),
    // );
  }

  await testFinalizeAndWrap();

  assertEquals(callCount, 2);
});
