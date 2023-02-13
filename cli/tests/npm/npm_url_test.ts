// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertRejects } from "./test_util.ts";

Deno.test(
  async function invalidImport() {
    await assertRejects(
      async () => await import("123"),
      TypeError,
      'Relative import path "123" not prefixed with / or ./ or ../',
    );
    await assertRejects(
      async () => await import("node:invalid"),
      TypeError,
      'Unknown built-in "node:" module: invalid',
    );
    // Test that deno does not crash on modules called "npm:ws:" and "npm:wss:"
    // TODO(#17802): update two tests after refactoring
    await assertRejects(
      async () => await import(`npm:${"ws:"}`),
      TypeError,
      'Error getting npm package "ws:"',
    );
    await assertRejects(
      async () => await import(`npm:${"other"}`),
      TypeError,
      'Error getting npm package "ws:"',
    );
  },
);
