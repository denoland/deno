//import * as ts from "typescript";
import { main as pb } from "./msg.pb"

V8Worker2.recv((ab: ArrayBuffer) => {
  let msg = pb.Msg.decode(new Uint8Array(ab));
  V8Worker2.print("msg.argv", msg.argv);
});

V8Worker2.print("Hello");
