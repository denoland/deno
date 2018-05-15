import { main as pb } from "./msg.pb";
import { TextDecoder } from "text-encoding";

// TODO move this to types.ts
type TypedArray = Uint8Array | Float32Array | Int32Array;

export function exit(code = 0): void {
  sendMsgFromObject({
    kind: pb.Msg.MsgKind.EXIT,
    code
  });
}

export function compileOutput(source: string, filename: string): void {
  sendMsgFromObject({
    kind: pb.Msg.MsgKind.COMPILE_OUTPUT,
    compileOutput: { source, filename }
  });
}

export function readFileSync(filename: string): string {
  const res = sendMsgFromObject({
    kind: pb.Msg.MsgKind.READ_FILE_SYNC,
    path: filename
  });
  if (res.error != null && res.error.length > 0) {
    throw Error(res.error);
  }
  const decoder = new TextDecoder("utf8");
  return decoder.decode(res.data);
}

function typedArrayToArrayBuffer(ta: TypedArray): ArrayBuffer {
  const ab = ta.buffer.slice(ta.byteOffset, ta.byteOffset + ta.byteLength);
  return ab as ArrayBuffer;
}

function sendMsgFromObject(obj: pb.IMsg): null | pb.Msg {
  const msg = pb.Msg.fromObject(obj);
  const ui8 = pb.Msg.encode(msg).finish();
  const ab = typedArrayToArrayBuffer(ui8);
  const resBuf = V8Worker2.send(ab);
  if (resBuf != null && resBuf.byteLength > 0) {
    return pb.Msg.decode(new Uint8Array(resBuf));
  } else {
    return null;
  }
}
