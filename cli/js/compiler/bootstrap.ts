// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { CompilerHostTarget, Host } from "./host.ts";
import { ASSETS } from "./sourcefile.ts";
import { getAsset } from "./util.ts";

// NOTE: target doesn't really matter here,
// this is in fact a mock host created just to
// load all type definitions and snapshot them.
const host = new Host({
  target: CompilerHostTarget.Main,
  writeFile(): void {},
});
const options = host.getCompilationSettings();

// This is a hacky way of adding our libs to the libs available in TypeScript()
// as these are internal APIs of TypeScript which maintain valid libs
ts.libs.push("deno.ns", "deno.window", "deno.worker", "deno.shared_globals");
ts.libMap.set("deno.ns", "lib.deno.ns.d.ts");
ts.libMap.set("deno.window", "lib.deno.window.d.ts");
ts.libMap.set("deno.worker", "lib.deno.worker.d.ts");
ts.libMap.set("deno.shared_globals", "lib.deno.shared_globals.d.ts");

// this pre-populates the cache at snapshot time of our library files, so they
// are available in the future when needed.
host.getSourceFile(`${ASSETS}/lib.deno.ns.d.ts`, ts.ScriptTarget.ESNext);
host.getSourceFile(`${ASSETS}/lib.deno.window.d.ts`, ts.ScriptTarget.ESNext);
host.getSourceFile(`${ASSETS}/lib.deno.worker.d.ts`, ts.ScriptTarget.ESNext);
host.getSourceFile(
  `${ASSETS}/lib.deno.shared_globals.d.ts`,
  ts.ScriptTarget.ESNext
);

export const TS_SNAPSHOT_PROGRAM = ts.createProgram({
  rootNames: [`${ASSETS}/bootstrap.ts`],
  options,
  host,
});

export const SYSTEM_LOADER = getAsset("system_loader.js");
