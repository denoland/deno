import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../../testing/asserts.ts";
import { realpath, realpathSync } from "./_fs_realpath.ts";

Deno.test("realpath", async function () {
  const tempFile = await Deno.makeTempFile();
  const tempFileAlias = tempFile + ".alias";
  await Deno.symlink(tempFile, tempFileAlias);
  const realPath = await new Promise((resolve, reject) => {
    realpath(tempFile, (err, path) => {
      if (err) {
        reject(err);
        return;
      }
      resolve(path);
    });
  });
  const realSymLinkPath = await new Promise((resolve, reject) => {
    realpath(tempFileAlias, (err, path) => {
      if (err) {
        reject(err);
        return;
      }
      resolve(path);
    });
  });
  assertEquals(realPath, realSymLinkPath);
});

Deno.test("realpathSync", function () {
  const tempFile = Deno.makeTempFileSync();
  const tempFileAlias = tempFile + ".alias";
  Deno.symlinkSync(tempFile, tempFileAlias);
  const realPath = realpathSync(tempFile);
  const realSymLinkPath = realpathSync(tempFileAlias);
  assertEquals(realPath, realSymLinkPath);
});

Deno.test("[std/node/fs] realpath callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  await Deno.writeTextFile(`${tempDir}/file.txt`, "hello world");
  await Deno.symlink(
    `${tempDir}/file.txt`,
    `${tempDir}/link.txt`,
    { type: "file" },
  );
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { realpath } from "${
        new URL("./_fs_realpath.ts", import.meta.url).href
      }";

      realpath(${JSON.stringify(`${tempDir}/link.txt`)}, (err) => {
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
  await Deno.remove(tempDir, { recursive: true });
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
