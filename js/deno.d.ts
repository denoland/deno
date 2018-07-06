// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
type MessageCallback = (channel: string, msg: ArrayBuffer) => void;

interface Deno {
  recv(cb: MessageCallback): void;
  send(channel: string, msg: ArrayBuffer): null | ArrayBuffer;
  print(x: string): void;
}

declare let deno: Deno;
