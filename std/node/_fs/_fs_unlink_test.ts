import {
  assert,
  assertEquals,
  assertStringIncludes,
  fail,
} from "../../testing/asserts.ts";
import { existsSync } from "../../fs/mod.ts";
import { unlink, unlinkSync } from "./_fs_unlink.ts";

Deno.test({
  name: "ASYNC: deleting a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<void>((resolve, reject) => {
      unlink(file, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(file), false), () => fail())
      .finally(() => {
        if (existsSync(file)) Deno.removeSync(file);
      });
  },
});

Deno.test({
  name: "SYNC: Test deleting a file",
  fn() {
    const file = Deno.makeTempFileSync();
    unlinkSync(file);
    assertEquals(existsSync(file), false);
  },
});

Deno.test("[std/node/fs] unlink callback isn't called twice if error is thrown", async () => {
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
      import { unlink } from "${
        new URL("./_fs_unlink.ts", import.meta.url).href
      }";

      unlink(${JSON.stringify(tempFile)}, (err) => {
        // If the bug is present and the callback is called again with an error,
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (!err) throw new Error("success");
      });`,
    ],
    stderr: "piped",
  });
  const status = await p.status();
  const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
  p.close();
  p.stderr.close();
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
