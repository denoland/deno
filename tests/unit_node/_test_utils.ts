// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertStringIncludes } from "@std/assert";

/** Asserts that an error thrown in a callback will not be wrongly caught. */
export async function assertCallbackErrorUncaught(
  { prelude, invocation, cleanup }: {
    /** Any code which needs to run before the actual invocation (notably, any import statements). */
    prelude?: string;
    /**
     * The start of the invocation of the function, e.g. `open("foo.txt", `.
     * The callback will be added after it.
     */
    invocation: string;
    /** Called after the subprocess is finished but before running the assertions, e.g. to clean up created files. */
    cleanup?: () => Promise<void> | void;
  },
) {
  // Since the error has to be uncaught, and that will kill the Deno process,
  // the only way to test this is to spawn a subprocess.
  const p = new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      `${prelude ?? ""}
  
        ${invocation}(err) => {
          // If the bug is present and the callback is called again with an error,
          // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
          if (!err) throw new Error("success");
        });`,
    ],
    stderr: "piped",
  });
  const { stderr, success } = await p.output();
  const error = new TextDecoder().decode(stderr);
  await cleanup?.();
  assert(!success);
  assertStringIncludes(error, "Error: success");
}
