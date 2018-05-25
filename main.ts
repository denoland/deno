import * as dispatch from "./dispatch";
import { main as pb } from "./msg.pb";

import * as runtime from "./runtime";
import * as util from "./util";

// These have top-level functions that need to execute.
import { initTimers } from "./timers";

// To control internal logging output
// Set with the -debug command-line flag.
export let debug = false;
let startCalled = false;

dispatch.sub("start", (payload: Uint8Array) => {
  if (startCalled) {
    throw Error("start message received more than once!");
  }
  startCalled = true;

  const msg = pb.Msg.decode(payload);
  const cwd = msg.startCwd;
  const argv = msg.startArgv;
  const debugFlag = msg.startDebugFlag;
  const mainJs = msg.startMainJs;
  const mainMap = msg.startMainMap;

  debug = debugFlag;
  util.log("start", { cwd, argv, debugFlag });

  initTimers();
  runtime.setup(mainJs, mainMap);

  const inputFn = argv[0];
  const mod = runtime.resolveModule(inputFn, cwd + "/");
  mod.compileAndRun();
});
