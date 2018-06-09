// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
// This allows us to have async/await in our code. It must be loaded first.
import "babel-polyfill";

import * as dispatch from "./dispatch";
import { main as pb } from "./msg.pb";

import * as runtime from "./runtime";
import * as util from "./util";

import { initTimers } from "./timers";
import { initFetch } from "./fetch";
import { initHttp } from "./http";

// To control internal logging output
// Set with the -debug command-line flag.
export let debug = false;
let startCalled = false;

// denoMain is needed to allow hooks into the system.
// Also eventual snapshot support needs it.
// tslint:disable-next-line:no-any
(window as any)["denoMain"] = () => {
  // tslint:disable-next-line:no-any
  delete (window as any)["denoMain"];

  initTimers();
  initFetch();
  initHttp();

  dispatch.sub("start", (payload: Uint8Array) => {
    if (startCalled) {
      throw Error("start message received more than once!");
    }
    startCalled = true;

    const msg = pb.Msg.decode(payload);
    const {
      startCwd: cwd,
      startArgv: argv,
      startDebugFlag: debugFlag,
      startMainJs: mainJs,
      startMainMap: mainMap
    } = msg;

    debug = debugFlag;
    util.log("start", { cwd, argv, debugFlag });

    runtime.setup(mainJs, mainMap);

    const inputFn = argv[0];
    const mod = runtime.resolveModule(inputFn, `${cwd}/`);
    mod.compileAndRun();
  });
};
