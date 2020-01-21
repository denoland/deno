// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { ASSETS, Host } from "./compiler_host.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";
import { initOps } from "./os.ts";

// This registers ops that are available during the snapshotting process.
initOps();

const host = new Host({ writeFile(): void {} });
const options = host.getCompilationSettings();

/** Used to generate the foundational AST for all other compilations, so it can
 * be cached as part of the snapshot and available to speed up startup */
export const oldProgram = ts.createProgram({
  rootNames: [`${ASSETS}/bootstrap.ts`],
  options,
  host
});

/** A module loader which is concatenated into bundle files.  We read all static
 * assets during the snapshotting process, which is why this is located in
 * compiler_bootstrap. */
export const bundleLoader = sendSync(dispatch.OP_FETCH_ASSET, {
  name: "bundle_loader.js"
});
