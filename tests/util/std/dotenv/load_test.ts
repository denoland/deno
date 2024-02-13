// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import * as path from "../path/mod.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = path.resolve(moduleDir, "testdata");

Deno.test({
  name: "load",
  async fn() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--allow-read",
        "--allow-env",
        path.join(testdataDir, "./app_load.ts"),
      ],
      clearEnv: true,
      cwd: testdataDir,
    });
    const { stdout } = await command.output();

    const decoder = new TextDecoder();
    assertEquals(
      decoder.decode(stdout).trim(),
      "hello world",
    );
  },
});

Deno.test({
  name: "load when multiple files",
  async fn() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--allow-read",
        "--allow-env",
        path.join(testdataDir, "./app_load_parent.ts"),
      ],
      clearEnv: true,
      cwd: testdataDir,
    });
    const { stdout } = await command.output();

    const decoder = new TextDecoder();
    assertEquals(
      decoder.decode(stdout).trim(),
      "hello world",
    );
  },
});
