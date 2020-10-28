import { assertEquals } from "../../testing/asserts.ts";
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
