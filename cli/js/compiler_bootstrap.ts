// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { ASSETS, CompilerHostTarget, Host } from "./compiler_host.ts";
import { core } from "./core.ts";
import * as dispatch from "./dispatch.ts";
import { getAsset } from "./compiler_util.ts";

// This registers ops that are available during the snapshotting process.
const ops = core.ops();
for (const [name, opId] of Object.entries(ops)) {
  const opName = `OP_${name.toUpperCase()}`;
  // TODO This type casting is dangerous, and should be improved when the same
  // code in `os.ts` is done.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (dispatch as any)[opName] = opId;
}

const host = new Host({
  target: CompilerHostTarget.Main,
  writeFile(): void {}
});
const options = host.getCompilationSettings();

// This is a hacky way of adding our libs to the libs available in TypeScript()
// as these are internal APIs of TypeScript which maintain valid libs
/* eslint-disable @typescript-eslint/no-explicit-any */
(ts as any).libs.push("deno_main", "deno_worker", "deno");
(ts as any).libMap.set("deno_main", "lib.deno_main.d.ts");
(ts as any).libMap.set("deno_worker", "lib.deno_worker.d.ts");
(ts as any).libMap.set("deno", "lib.deno.d.ts");
/* eslint-enable @typescript-eslint/no-explicit-any */

// this pre-populates the cache at snapshot time of our library files, so they
// are available in the future when needed.
host.getSourceFile(`${ASSETS}/lib.deno_main.d.ts`, ts.ScriptTarget.ESNext);
host.getSourceFile(`${ASSETS}/lib.deno_worker.d.ts`, ts.ScriptTarget.ESNext);
host.getSourceFile(`${ASSETS}/lib.deno.d.ts`, ts.ScriptTarget.ESNext);
host.getSourceFile(`${ASSETS}/lib.webworker.d.ts`, ts.ScriptTarget.ESNext);

/** Used to generate the foundational AST for all other compilations, so it can
 * be cached as part of the snapshot and available to speed up startup */
export const oldProgram = ts.createProgram({
  rootNames: [`${ASSETS}/bootstrap.ts`],
  options,
  host
});

/** A module loader which is concatenated into bundle files.
 *
 * We read all static assets during the snapshotting process, which is
 * why this is located in compiler_bootstrap.
 **/
export const bundleLoader = getAsset("bundle_loader.js");
