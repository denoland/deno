// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../../testing/asserts.ts";
import { exists, existsSync } from "./_fs_exists.ts";
import { promisify } from "../util.ts";

Deno.test("[std/node/fs] exists", async function () {
  const availableFile = await new Promise((resolve) => {
    const tmpFilePath = Deno.makeTempFileSync();
    exists(tmpFilePath, (exists: boolean) => {
      Deno.removeSync(tmpFilePath);
      resolve(exists);
    });
  });
  const notAvailableFile = await new Promise((resolve) => {
    exists("./notAvailable.txt", (exists: boolean) => resolve(exists));
  });
  assertEquals(availableFile, true);
  assertEquals(notAvailableFile, false);
});

Deno.test("[std/node/fs] existsSync", function () {
  const tmpFilePath = Deno.makeTempFileSync();
  assertEquals(existsSync(tmpFilePath), true);
  Deno.removeSync(tmpFilePath);
  assertEquals(existsSync("./notAvailable.txt"), false);
});

Deno.test("[std/node/fs] promisify(exists)", async () => {
  const tmpFilePath = await Deno.makeTempFile();
  try {
    const existsPromisified = promisify(exists);
    assert(await existsPromisified(tmpFilePath));
    assert(!await existsPromisified("./notAvailable.txt"));
  } finally {
    await Deno.remove(tmpFilePath);
  }
});

Deno.test("[std/node/fs] exists callback isn't called twice if error is thrown", async () => {
  // This doesn't use `assertCallbackErrorUncaught()` because `exists()` doesn't return a standard node callback, which is what it expects.
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("./_fs_exists.ts", import.meta.url);
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      "--no-check",
      `
      import { exists } from ${JSON.stringify(importUrl)};

      exists(${JSON.stringify(tempFile)}, (exists) => {
        // If the bug is present and the callback is called again with false (meaning an error occurred),
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (exists) throw new Error("success");
      });`,
    ],
  });
  const { success, stderr } = await command.output();
  await Deno.remove(tempFile);
  assert(!success);
  assertStringIncludes(new TextDecoder().decode(stderr), "Error: success");
});
