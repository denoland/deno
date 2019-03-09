// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// eslint-disable-next-line @typescript-eslint/no-triple-slash-reference
/// <reference path="./plugins.d.ts" />

import "./globals";

import { assert, log } from "./util";
import * as os from "./os";
import { args } from "./deno";
import { replLoop } from "./repl";
import { setVersions } from "./version";
import { setLocation } from "./location";

// builtin modules
import * as deno from "./deno";

// TODO(kitsonk) remove with `--types` below
import libDts from "gen/lib/lib.deno_runtime.d.ts!string";

export default function denoMain(): void {
  const startResMsg = os.start();

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

  const mainModule = startResMsg.mainModule();
  if (mainModule) {
    assert(mainModule.length > 0);
    setLocation(mainModule);
  }

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  for (let i = 1; i < startResMsg.argvLength(); i++) {
    args.push(startResMsg.argv(i));
  }
  log("args", args);
  Object.freeze(args);

  if (!mainModule) {
    replLoop();
  }
}
