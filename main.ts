//import * as ts from "typescript";
import { main as pb } from "./msg.pb"
import "./util";
import { TextDecoder } from "text-encoding";

function readFileSync(filename: string): string {
  const msg = pb.Msg.fromObject({
    kind: pb.Msg.MsgKind.READ_FILE_SYNC,
    path: filename
  });
  const ui8 = pb.Msg.encode(msg).finish();
  const ab = ui8.buffer.slice(ui8.byteOffset, ui8.byteOffset + ui8.byteLength);
  const resBuf = V8Worker2.send(ab as ArrayBuffer);
  const res = pb.Msg.decode(new Uint8Array(resBuf));
  if (res.error != null && res.error.length > 0) {
    throw Error(res.error);
  }
  const decoder = new TextDecoder("utf8");
  return decoder.decode(res.data)
}

function load(argv: string[]): void {
  console.log("Load argv", argv);
  const inputFn = argv[1];
  const source = readFileSync(inputFn);
  console.log("source", source)
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
