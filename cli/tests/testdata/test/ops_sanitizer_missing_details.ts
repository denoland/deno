// https://github.com/denoland/deno/issues/13729
// https://github.com/denoland/deno/issues/13938
import { writeAll } from "../../../../test_util/std/io/util.ts";

Deno.test("test 1", { permissions: { write: true, read: true } }, async () => {
  const tmpFile = await Deno.makeTempFile();
  const file = await Deno.open(tmpFile, { write: true });
  const buf = new Uint8Array(new Array(1_000_000).fill(1));
  writeAll(file, buf);
});
