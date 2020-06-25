// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { tempFile } from "./util.ts";

Deno.test({
  name: "[io/util] tempfile",
  fn: async function (): Promise<void> {
    const f = await tempFile(".", {
      prefix: "prefix-",
      postfix: "-postfix",
    });
    const base = path.basename(f.filepath);
    assert(!!base.match(/^prefix-.+?-postfix$/));
    f.file.close();
    await Deno.remove(f.filepath);
  },
});
