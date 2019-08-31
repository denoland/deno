// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import "./globals.ts";

import { assert, log } from "./util.ts";
import * as os from "./os.ts";
import { args } from "./deno.ts";
import { setPrepareStackTrace } from "./error_stack.ts";
import { replLoop } from "./repl.ts";
import { xevalMain, XevalFunc } from "./xeval.ts";
import { setVersions } from "./version.ts";
import { window } from "./window.ts";
import { setLocation } from "./location.ts";
import * as Deno from "./deno.ts";

function denoMain(preserveDenoNamespace: boolean = true, name?: string): void {
  const s = os.start(preserveDenoNamespace, name);

  setVersions(s.denoVersion, s.v8Version);

  // handle `--version`
  if (s.versionFlag) {
    const { console } = window;
    console.log("deno:", Deno.version.deno);
    console.log("v8:", Deno.version.v8);
    console.log("typescript:", Deno.version.typescript);
    os.exit(0);
  }

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

  if (window["_xevalWrapper"] !== undefined) {
    xevalMain(window["_xevalWrapper"] as XevalFunc, s.xevalDelim);
  } else if (!s.mainModule) {
    replLoop();
  }
}
window["denoMain"] = denoMain;
