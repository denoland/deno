import { main as pb } from "./msg.pb";
import "./util";
import { compile } from "./compiler";

function start(cwd: string, argv: string[]): void {
  // TODO parse arguments.
  const inputFn = argv[1];
  compile(cwd, inputFn);
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
