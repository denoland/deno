import { main as pb } from "./msg.pb";
import { TextDecoder } from "text-encoding";

export function readFileSync(filename: string): string {
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
  return decoder.decode(res.data);
}
