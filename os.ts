import { main as pb } from "./msg.pb";

// TODO move this to types.ts
type TypedArray = Uint8Array | Float32Array | Int32Array;

export function exit(code = 0): void {
  sendMsgFromObject({
    exit: { code }
  });
}

export function sourceCodeFetch(
  filename: string
): { sourceCode: string; outputCode: string } {
  const res = sendMsgFromObject({
    sourceCodeFetch: { filename }
  });
  const { sourceCode, outputCode } = res.sourceCodeFetchRes;
  return { sourceCode, outputCode };
}

export function sourceCodeCache(
  filename: string,
  sourceCode: string,
  outputCode: string
): void {
  const res = sendMsgFromObject({
    sourceCodeCache: { filename, sourceCode, outputCode }
  });
  throwOnError(res);
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
    const res = pb.Msg.decode(new Uint8Array(resBuf));
    throwOnError(res);
    return res;
  } else {
    return null;
  }
}

function throwOnError(res: pb.Msg) {
  if (res != null && res.error != null && res.error.length > 0) {
    throw Error(res.error);
  }
}
