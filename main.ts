//import * as ts from "typescript";
import { main as pb } from "./msg.pb"
import "./util";


function load(argv: string[]): void {
  console.log("Load argv", argv);
}

V8Worker2.recv((ab: ArrayBuffer) => {
  const msg = pb.Msg.decode(new Uint8Array(ab));
  switch (msg.kind) {
      case pb.Msg.MsgKind.LOAD:
        load(msg.argv);
        break;
      default:
        console.log("Unknown message", msg);
        break;
  }
});

V8Worker2.print("Hello");
