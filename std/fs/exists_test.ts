// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertStringIncludes } from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { exists, existsSync } from "./exists.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = path.resolve(moduleDir, "testdata");

Deno.test("[fs] existsFile", async function (): Promise<void> {
  assertEquals(
    await exists(path.join(testdataDir, "not_exist_file.ts")),
    false,
  );
  assertEquals(await existsSync(path.join(testdataDir, "0.ts")), true);
});

Deno.test("[fs] existsFileSync", function (): void {
  assertEquals(existsSync(path.join(testdataDir, "not_exist_file.ts")), false);
  assertEquals(existsSync(path.join(testdataDir, "0.ts")), true);
});

Deno.test("[fs] existsDirectory", async function (): Promise<void> {
  assertEquals(
    await exists(path.join(testdataDir, "not_exist_directory")),
    false,
  );
  assertEquals(existsSync(testdataDir), true);
});

Deno.test("[fs] existsDirectorySync", function (): void {
  assertEquals(
    existsSync(path.join(testdataDir, "not_exist_directory")),
    false,
  );
  assertEquals(existsSync(testdataDir), true);
});

Deno.test("[fs] existsLinkSync", function (): void {
  // TODO(axetroy): generate link file use Deno api instead of set a link file
  // in repository
  assertEquals(existsSync(path.join(testdataDir, "0-link")), true);
});

Deno.test("[fs] existsLink", async function (): Promise<void> {
  // TODO(axetroy): generate link file use Deno api instead of set a link file
  // in repository
  assertEquals(await exists(path.join(testdataDir, "0-link")), true);
});

interface Scenes {
  read: boolean; // --allow-read
  async: boolean;
  output: string;
  file: string; // target file to run
}

const scenes: Scenes[] = [
  // 1
  {
    read: false,
    async: true,
    output: "run again with the --allow-read flag",
    file: "0.ts",
  },
  {
    read: false,
    async: false,
    output: "run again with the --allow-read flag",
    file: "0.ts",
  },
  // 2
  {
    read: true,
    async: true,
    output: "exist",
    file: "0.ts",
  },
  {
    read: true,
    async: false,
    output: "exist",
    file: "0.ts",
  },
  // 3
  {
    read: false,
    async: true,
    output: "run again with the --allow-read flag",
    file: "no_exist_file_for_test.ts",
  },
  {
    read: false,
    async: false,
    output: "run again with the --allow-read flag",
    file: "no_exist_file_for_test.ts",
  },
  // 4
  {
    read: true,
    async: true,
    output: "not exist",
    file: "no_exist_file_for_test.ts",
  },
  {
    read: true,
    async: false,
    output: "not exist",
    file: "no_exist_file_for_test.ts",
  },
];

for (const s of scenes) {
  let title = `test ${s.async ? "exists" : "existsSync"}("testdata/${s.file}")`;
  title += ` ${s.read ? "with" : "without"} --allow-read`;
  Deno.test(`[fs] existsPermission ${title}`, async function (): Promise<void> {
    const args = [Deno.execPath(), "run"];

    if (s.read) {
      args.push("--allow-read");
    }

    args.push(path.join(testdataDir, s.async ? "exists.ts" : "exists_sync.ts"));
    args.push(s.file);

    const p = Deno.run({
      stdout: "piped",
      cwd: testdataDir,
      cmd: args,
    });

    const output = await p.output();
    p.close();
    assertStringIncludes(new TextDecoder().decode(output), s.output);
  });
  // done
}
