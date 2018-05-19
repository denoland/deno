import { main as pb } from "./msg.pb";
import "./util";
import * as runtime from "./runtime";
import * as timers from "./timers";
import * as util from "./util";
import * as sourceMap from "source-map-support";

function retrieveFileForSourceMap(path: string): string {
  util.log("retrieveFileForSourceMap", path);
  throw Error("implemtn me");
}

function retrieveSourceMap(source: string): sourceMap.UrlAndMap {
  util.log("retrieveSourceMap", source);
  throw Error("implemtn me");
  //return { url, map }
}

// TODO handleUncaughtExceptions
sourceMap.install({
  //handleUncaughtExceptions: true,
  environment: "node",
  retrieveFile: retrieveFileForSourceMap,
  hookRequire: false,
  overrideRetrieveSourceMap: true,
  overrideRetrieveFile: true,
  retrieveSourceMap
});

// To control internal logging output
// Set with the -debug command-line flag.
export let debug = false;

function start(cwd: string, argv: string[], debugFlag: boolean): void {
  debug = debugFlag;
  util.log("start", { cwd, argv, debugFlag });

  const inputFn = argv[0];
  const mod = runtime.resolveModule(inputFn, cwd + "/");
  mod.compileAndRun();
}

V8Worker2.recv((ab: ArrayBuffer) => {
  const msg = pb.Msg.decode(new Uint8Array(ab));
  switch (msg.payload) {
    case "start":
      start(msg.start.cwd, msg.start.argv, msg.start.debugFlag);
      break;
    case "timerReady":
      timers.timerReady(msg.timerReady.id, msg.timerReady.done);
      break;
    default:
      console.log("Unknown message", msg);
      break;
  }
});
