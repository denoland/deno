// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../../testing/asserts.ts";
import { exists, existsSync } from "./_fs_exists.ts";

Deno.test("existsFile", async function () {
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

Deno.test("existsSyncFile", function () {
  const tmpFilePath = Deno.makeTempFileSync();
  assertEquals(existsSync(tmpFilePath), true);
  Deno.removeSync(tmpFilePath);
  assertEquals(existsSync("./notAvailable.txt"), false);
});

Deno.test("[std/node/fs] exists callback isn't called twice if error is thrown", async () => {
  // This doesn't use `assertCallbackErrorUncaught()` because `exists()` doesn't return a standard node callback, which is what it expects.
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("./_fs_exists.ts", import.meta.url);
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { exists } from ${JSON.stringify(importUrl)};

      exists(${JSON.stringify(tempFile)}, (exists) => {
        // If the bug is present and the callback is called again with false (meaning an error occured),
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (exists) throw new Error("success");
      });`,
    ],
    stderr: "piped",
  });
  const status = await p.status();
  const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
  p.close();
  p.stderr.close();
  await Deno.remove(tempFile);
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
