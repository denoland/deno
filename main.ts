import { main as pb } from "./msg.pb";
import "./util";
import * as runtime from "./runtime";
import * as path from "path";

function start(cwd: string, argv: string[]): void {
  // TODO parse arguments.
  const inputFn = argv[1];
  const fn = path.resolve(cwd, inputFn);
  const m = runtime.FileModule.load(fn);
  m.compileAndRun();
}

V8Worker2.recv((ab: ArrayBuffer) => {
  const msg = pb.Msg.decode(new Uint8Array(ab));
  switch (msg.kind) {
    case pb.Msg.MsgKind.START:
      start(msg.start.cwd, msg.start.argv);
      break;
    default:
      console.log("Unknown message", msg);
      break;
  }
});
