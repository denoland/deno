// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { ASSETS, Host } from "./compiler_host.ts";
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

const host = new Host({ writeFile(): void {} });
const options = host.getCompilationSettings();

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
