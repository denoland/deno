import {
  assert,
  assertEquals,
  assertNotEquals,
  assertStringIncludes,
  fail,
} from "../../testing/asserts.ts";
import { readdir, readdirSync } from "./_fs_readdir.ts";
import { join } from "../../path/mod.ts";

Deno.test({
  name: "ASYNC: reading empty directory",
  async fn() {
    const dir = Deno.makeTempDirSync();
    await new Promise<string[]>((resolve, reject) => {
      readdir(dir, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    })
      .then((files) => assertEquals(files, []), () => fail())
      .finally(() => Deno.removeSync(dir));
  },
});

function assertEqualsArrayAnyOrder<T>(actual: T[], expected: T[]) {
  assertEquals(actual.length, expected.length);
  for (const item of expected) {
    const index = actual.indexOf(item);
    assertNotEquals(index, -1);
    expected = expected.splice(index, 1);
  }
}

Deno.test({
  name: "ASYNC: reading non-empty directory",
  async fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    await new Promise<string[]>((resolve, reject) => {
      readdir(dir, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    })
      .then(
        (files) =>
          assertEqualsArrayAnyOrder(
            files,
            ["file1.txt", "some_dir", "file2.txt"],
          ),
        () => fail(),
      )
      .finally(() => Deno.removeSync(dir, { recursive: true }));
  },
});

Deno.test({
  name: "SYNC: reading empty the directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    assertEquals(readdirSync(dir), []);
  },
});

Deno.test({
  name: "SYNC: reading non-empty directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    assertEqualsArrayAnyOrder(
      readdirSync(dir),
      ["file1.txt", "some_dir", "file2.txt"],
    );
  },
});

Deno.test("[std/node/fs] readdir callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { readdir } from "${
        new URL("./_fs_readdir.ts", import.meta.url).href
      }";

      readdir(${JSON.stringify(tempDir)}, (err) => {
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
  await Deno.remove(tempDir);
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
