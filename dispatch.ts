import { typedArrayToArrayBuffer } from "./util";
import { _global } from "./globals";
import { main as pb } from "./msg.pb";

type MessageCallback = (msg: Uint8Array) => void;

const send = V8Worker2.send;
const channels = new Map<string, MessageCallback[]>();

export function sub(channel: string, cb: MessageCallback): void {
  let subscribers = channels.get(channel);
  if (!subscribers) {
    subscribers = [];
    channels.set(channel, subscribers);
  }
  subscribers.push(cb);
}

export function pub(channel: string, payload: Uint8Array): null | ArrayBuffer {
  const msg = pb.BaseMsg.fromObject({ channel, payload });
  const ui8 = pb.BaseMsg.encode(msg).finish();
  const ab = typedArrayToArrayBuffer(ui8);
  return send(ab);
}

// Internal version of "pub".
// TODO add internal version of "sub"
// TODO rename to pubInternal()
export function sendMsgFromObject(
  channel: string,
  obj: pb.IMsg
): null | pb.Msg {
  const msg = pb.Msg.fromObject(obj);
  const ui8 = pb.Msg.encode(msg).finish();
  const resBuf = pub(channel, ui8);
  if (resBuf != null && resBuf.byteLength > 0) {
    const res = pb.Msg.decode(new Uint8Array(resBuf));
    if (res != null && res.error != null && res.error.length > 0) {
      throw Error(res.error);
    }
    return res;
  } else {
    return null;
  }
}

V8Worker2.recv((ab: ArrayBuffer) => {
  const msg = pb.BaseMsg.decode(new Uint8Array(ab));
  const subscribers = channels.get(msg.channel);
  if (subscribers == null) {
    throw Error(`No subscribers for channel "${msg.channel}".`);
  }

  for (const subscriber of subscribers) {
    subscriber(msg.payload);
  }
});

// Delete the V8Worker2 from the global object, so that no one else can receive
// messages.
_global["V8Worker2"] = null;
