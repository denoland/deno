// Copyright 2018-2025 the Deno authors. MIT license.
import { throwCustomError } from "checkin:error";
import { assert, assertEquals, test } from "checkin:testing";

test(function testCustomError() {
  try {
    throwCustomError("uh oh");
  } catch (e) {
    assertEquals(e.message, "uh oh");
    assert(e instanceof Deno.core.BadResource);
  }
});

test(function testJsErrorConstructors() {
  const error = new Error("message");
  const badResource = new Deno.core.BadResource("bad resource", {
    cause: error,
  });
  assertEquals(badResource.message, "bad resource");
  assertEquals(badResource.cause, error);

  const Interrupted = new Deno.core.Interrupted("interrupted", {
    cause: error,
  });
  assertEquals(Interrupted.message, "interrupted");
  assertEquals(Interrupted.cause, error);

  const notCapable = new Deno.core.NotCapable("not capable", { cause: error });
  assertEquals(notCapable.message, "not capable");
  assertEquals(notCapable.cause, error);
});
