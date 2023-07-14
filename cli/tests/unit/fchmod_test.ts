// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { consoleSize } from "../../../runtime/js/40_tty.js";
import { assert, assertEquals } from "./test_util.ts";

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { read: true, write: true },
  },
  async function fchmodSyncSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });

    const file = await Deno.open(filename);
    console.log("cli/tests/unit/fchmod_test.ts", file.rid)

    Deno.fchmodSync(file.rid, 0o777);

    // const fileInfo = Deno.statSync(filename);
    // assert(fileInfo.mode);
    // assertEquals(fileInfo.mode & 0o777, 0o777);

    file.close();
  },
);
