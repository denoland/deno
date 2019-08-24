// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// eslint-disable-next-line @typescript-eslint/no-triple-slash-reference
/// <reference path="./plugins.d.ts" />

import "./globals";

import { assert, log } from "./util";
import * as os from "./os";
import { args } from "./deno";
import { setPrepareStackTrace } from "./error_stack";
import { replLoop } from "./repl";
import { xevalMain, XevalFunc } from "./xeval";
import { setVersions } from "./version";
import { window } from "./window";
import { setLocation } from "./location";

// builtin modules
import * as deno from "./deno";

export default function denoMain(
  preserveDenoNamespace: boolean = true,
  name?: string
): void {
  const s = os.start(preserveDenoNamespace, name);

  setVersions(s.denoVersion, s.v8Version);

  // handle `--version`
  if (s.versionFlag) {
    console.log("deno:", deno.version.deno);
    console.log("v8:", deno.version.v8);
    console.log("typescript:", deno.version.typescript);
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
