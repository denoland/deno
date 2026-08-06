// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright © Benjamin Lupton
// This code has been forked by https://github.com/bevry/domain-browser/commit/8bce7f4a093966ca850da75b024239ad5d0b33c6

import domain from "node:domain";
import { EventEmitter } from "node:events";
import { assertEquals } from "@std/assert";

const processWithDomain = process as typeof process & {
  domain: unknown | null;
};

function assertActiveDomain(expected: unknown | null) {
  assertEquals(processWithDomain.domain, expected);
}

function assertNoActiveDomain() {
  assertActiveDomain(null);

  // Entering and exiting a new domain must not reveal an older stack entry.
  const probe = domain.create();
  probe.enter();
  probe.exit();
  assertActiveDomain(null);
}

function checkNoDomainOnLaterTurn() {
  // Create the promise reaction before entering a domain so the check itself
  // is not associated with one of the domains being checked.
  const deferred = Promise.withResolvers<void>();
  return {
    check: deferred.promise.then(assertNoActiveDomain),
    schedule: deferred.resolve,
  };
}

Deno.test("should work on throws", async function () {
  const deferred = Promise.withResolvers<void>();
  const d = domain.create();

  d.on("error", function (err) {
    // @ts-ignore node:domain types are out of date
    assertEquals(err && err.message, "a thrown error", "error message");
    deferred.resolve();
  });
  d.run(function () {
    throw new Error("a thrown error");
  });
  await deferred.promise;
});

Deno.test("should be able to add emitters", async function () {
  const deferred = Promise.withResolvers<void>();
  const d = domain.create();
  const emitter = new EventEmitter();

  d.add(emitter);
  d.on("error", function (err) {
    assertEquals(err && err.message, "an emitted error", "error message");
    deferred.resolve();
  });

  emitter.emit("error", new Error("an emitted error"));
  await deferred.promise;
});

Deno.test("should be able to remove emitters", async function () {
  const deferred = Promise.withResolvers<void>();
  const emitter = new EventEmitter();
  const d = domain.create();
  let domainGotError = false;

  d.add(emitter);
  d.on("error", function (_err) {
    domainGotError = true;
  });

  emitter.on("error", function (err) {
    assertEquals(
      err && err.message,
      "This error should not go to the domain",
      "error message",
    );

    // Make sure nothing race condition-y is happening
    setTimeout(function () {
      assertEquals(domainGotError, false, "no domain error");
      deferred.resolve();
    }, 0);
  });

  d.remove(emitter);
  emitter.emit("error", new Error("This error should not go to the domain"));
  await deferred.promise;
});

Deno.test("bind should work", async function () {
  const deferred = Promise.withResolvers<void>();
  const d = domain.create();

  d.on("error", function (err) {
    assertEquals(err && err.message, "a thrown error", "error message");
    deferred.resolve();
  });
  d.bind(function (err: Error, a: number, b: number) {
    assertEquals(err && err.message, "a passed error", "error message");
    assertEquals(a, 2, "value of a");
    assertEquals(b, 3, "value of b");
    throw new Error("a thrown error");
  })(new Error("a passed error"), 2, 3);
  await deferred.promise;
});

Deno.test("intercept should work", async function () {
  const deferred = Promise.withResolvers<void>();
  const d = domain.create();
  let count = 0;
  d.on("error", function (err) {
    if (count === 0) {
      assertEquals(err && err.message, "a thrown error", "error message");
    } else if (count === 1) {
      assertEquals(err && err.message, "a passed error", "error message");
      deferred.resolve();
    }
    count++;
  });

  d.intercept(function (a: number, b: number) {
    assertEquals(a, 2, "value of a");
    assertEquals(b, 3, "value of b");
    throw new Error("a thrown error");
    // @ts-ignore node:domain types are out of date
  })(null, 2, 3);

  d.intercept(function (_a: number, _b: number) {
    throw new Error("should never reach here");
    // @ts-ignore node:domain types are out of date
  })(new Error("a passed error"), 2, 3);
  await deferred.promise;
});

Deno.test("handled async errors clear manually entered domains", async () => {
  const outer = domain.create();
  const inner = domain.create();
  const laterTurn = checkNoDomainOnLaterTurn();

  inner.on("error", (error) => {
    assertEquals(error.message, "inner callback failed");
    laterTurn.schedule();
  });

  outer.run(() => {
    setTimeout(() => {
      inner.enter();
      throw new Error("inner callback failed");
    }, 0);
  });

  await laterTurn.check;
});

Deno.test("errors from a nested domain handler reach its parent", async () => {
  const parent = domain.create();
  const child = domain.create();
  const laterTurn = checkNoDomainOnLaterTurn();

  parent.on("error", (error) => {
    assertEquals(error.message, "child handler failed");
    assertActiveDomain(null);
    laterTurn.schedule();
  });
  child.on("error", () => {
    assertActiveDomain(parent);
    throw new Error("child handler failed");
  });

  parent.run(() => {
    setTimeout(() => {
      child.enter();
      throw new Error("child callback failed");
    }, 0);
  });

  await laterTurn.check;
});

Deno.test("balanced domain entries preserve their parent", () => {
  const parent = domain.create();
  const child = domain.create();

  parent.enter();
  try {
    child.enter();
    try {
      assertActiveDomain(child);
    } finally {
      child.exit();
    }

    assertActiveDomain(parent);
  } finally {
    parent.exit();
  }

  assertNoActiveDomain();
});
