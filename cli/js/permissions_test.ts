// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(async function permissionInvalidName(): Promise<void> {
  let thrown = false;
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    await Deno.permissions.query({ name: "foo" as any });
  } catch (e) {
    thrown = true;
    assert(e instanceof Error);
  } finally {
    assert(thrown);
  }
});

test(async function permissionNetInvalidUrl(): Promise<void> {
  let thrown = false;
  try {
    await Deno.permissions.query({ name: "net", url: ":" });
  } catch (e) {
    thrown = true;
    assert(e instanceof URIError);
  } finally {
    assert(thrown);
  }
});
