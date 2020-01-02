// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import "./globals.ts";

import { assert, log } from "./util.ts";
import * as os from "./os.ts";
import { args } from "./deno.ts";
import { setPrepareStackTrace } from "./error_stack.ts";
import { replLoop } from "./repl.ts";
import { setVersions } from "./version.ts";
import { window } from "./window.ts";
import { setLocation } from "./location.ts";
import { setBuildInfo } from "./build.ts";
import { setSignals } from "./process.ts";

function denoMain(preserveDenoNamespace = true, name?: string): void {
  const s = os.start(preserveDenoNamespace, name);

  setBuildInfo(s.os, s.arch);
  setSignals();
  setVersions(s.denoVersion, s.v8Version, s.tsVersion);

  setPrepareStackTrace(Error);

  if (s.mainModule) {
    assert(s.mainModule.length > 0);
    setLocation(s.mainModule);
  }

  log("cwd", s.cwd);

  for (let i = 1; i < s.argv.length; i++) {
    args.push(s.argv[i]);
  }
  log("args", args);
  Object.freeze(args);

  if (!s.mainModule) {
    replLoop();
  }
}
window["denoMain"] = denoMain;
