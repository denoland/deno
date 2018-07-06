// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
import { typedArrayToArrayBuffer } from "./util";
import { deno as fbs } from "./msg_generated";

export type MessageCallback = (msg: Uint8Array) => void;
//type MessageStructCallback = (msg: pb.IMsg) => void;

const channels = new Map<string, MessageCallback[]>();

export function sub(channel: string, cb: MessageCallback): void {
  let subscribers = channels.get(channel);
  if (!subscribers) {
    subscribers = [];
    channels.set(channel, subscribers);
  }
  subscribers.push(cb);
}

deno.recv((channel: string, ab: ArrayBuffer) => {
  const subscribers = channels.get(channel);
  if (subscribers == null) {
    throw Error(`No subscribers for channel "${channel}".`);
  }

  const ui8 = new Uint8Array(ab);
  for (const subscriber of subscribers) {
    subscriber(ui8);
  }
});
