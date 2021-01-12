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
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempFile = await Deno.makeTempFile();
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { exists } from "${
        new URL("./_fs_exists.ts", import.meta.url).href
      }";

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
  await Deno.remove(tempFile, { recursive: true });
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
