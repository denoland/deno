// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertThrows } from "../testing/asserts.ts";

test("[hash/error] testCreateInvalidAlgorithm", () => {
  assertThrows(
    () => {
      Deno.createHash("wtf");
    },
    TypeError,
    "Unknown hash algorithm"
  );
});

test("[hash/error] testUpdateInvalidHash", () => {
  assertThrows(
    () => {
      Deno.updateHash(1234, new Uint8Array(1));
    },
    Deno.errors.BadResource,
    "Bad resource ID"
  );
});

test("[hash/error] testDigestInvalidHash", () => {
  assertThrows(
    () => {
      Deno.digestHash(1234);
    },
    Deno.errors.BadResource,
    "Bad resource ID"
  );
});
