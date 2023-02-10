// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { writeBuffer } from "./node_file.ts";
import { assertEquals } from "../../testing/asserts.ts";
import { isWindows } from "../../_util/os.ts";

Deno.test("[node/internal_binding/node_file] writeBuffer", async () => {
  const tempfile = await Deno.makeTempFile();
  await Deno.writeTextFile(tempfile, "01234567890123456789");

  const file = await Deno.open(tempfile, { write: true });
  await Deno.seek(file.rid, 5, Deno.SeekMode.Start);

  try {
    const ctx: { errno?: number } = {};
    const bytesWritten = writeBuffer(
      file.rid,
      new Uint8Array([65, 66, 67, 68, 69] /* abcde */),
      1,
      3,
      5,
      ctx,
    );

    assertEquals(await Deno.readTextFile(tempfile), "0123456789BCD3456789");
    assertEquals(bytesWritten, 3);
    assertEquals(typeof ctx.errno, "undefined");
  } finally {
    Deno.close(file.rid);
    await Deno.remove(tempfile);
  }
});
Deno.test("[node/internal_binding/node_file] writeBuffer", async () => {
  const tempfile = await Deno.makeTempFile();
  await Deno.writeTextFile(tempfile, "01234567890123456789");
  const file = await Deno.open(tempfile, { read: true, write: false });

  try {
    const ctx: { errno?: number } = {};
    const bytesWritten = writeBuffer(
      file.rid,
      new Uint8Array([65, 66, 67, 68, 69] /* abcde */),
      1,
      3,
      5,
      ctx,
    );
    assertEquals(bytesWritten, 0);
    if (isWindows) {
      assertEquals(ctx.errno, 5); // Access is denied
    } else {
      assertEquals(ctx.errno, 9); // Bad file descriptor
    }
  } finally {
    Deno.close(file.rid);
    await Deno.remove(tempfile);
  }
});
