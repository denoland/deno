// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
type MessageCallback = (msg: ArrayBuffer) => void;

interface Deno {
  sub(channel: string, cb: MessageCallback): void;
  pub(channel: string, msg: ArrayBuffer): null | ArrayBuffer;
  print(x: string): void;
}

declare let deno: Deno;
