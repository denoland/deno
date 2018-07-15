// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
type MessageCallback = (msg: Uint8Array) => void;

interface Deno {
  recv(cb: MessageCallback): void;
  send(msg: ArrayBufferView): null | Uint8Array;
  print(x: string): void;
}

declare let deno: Deno;
