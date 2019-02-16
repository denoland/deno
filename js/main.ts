// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// tslint:disable-next-line:no-reference
/// <reference path="./plugins.d.ts" />

import "./globals";

import { log } from "./util";
import * as os from "./os";
import { libdeno } from "./libdeno";
import { args } from "./deno";
import { replLoop } from "./repl";
import { setVersions } from "./version";

// builtin modules
import * as deno from "./deno";

// TODO(kitsonk) remove with `--types` below
import libDts from "gen/lib/lib.deno_runtime.d.ts!string";

/* tslint:disable-next-line:no-default-export */
export default function denoMain() {
  const startResMsg = os.start();

  // TODO(kitsonk) remove when import "deno" no longer supported
  libdeno.builtinModules["deno"] = deno;
  Object.freeze(libdeno.builtinModules);

  setVersions(startResMsg.denoVersion()!, startResMsg.v8Version()!);

  // handle `--version`
  if (startResMsg.versionFlag()) {
    console.log("deno:", deno.version.deno);
    console.log("v8:", deno.version.v8);
    console.log("typescript:", deno.version.typescript);
    os.exit(0);
  }

  // handle `--types`
  // TODO(kitsonk) move to Rust fetching from compiler
  if (startResMsg.typesFlag()) {
    console.log(libDts);
    os.exit(0);
  }

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  for (let i = 1; i < startResMsg.argvLength(); i++) {
    args.push(startResMsg.argv(i));
  }
  log("args", args);
  Object.freeze(args);

  const inputFn = args[0];
  if (!inputFn) {
    replLoop();
  }
}
