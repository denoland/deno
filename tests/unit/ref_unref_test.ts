// Copyright 2018-2025 the Deno authors. MIT license.

import { assertNotEquals, execCode } from "./test_util.ts";

Deno.test("[unrefOpPromise] unref'ing invalid ops does not have effects", async () => {
  const [statusCode, _] = await execCode(`
    Deno[Deno.internal].core.unrefOpPromise(new Promise(r => null));
    setTimeout(() => { throw new Error() }, 10)
  `);
  // Invalid unrefOpPromise call doesn't affect exit condition of event loop
  assertNotEquals(statusCode, 0);
});
