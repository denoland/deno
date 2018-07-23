// Copyright 2018 the Deno authors. All rights reserved. MIT license.
type MessageCallback = (msg: Uint8Array) => void;

interface Deno {
  recv(cb: MessageCallback): void;
  send(msg: ArrayBufferView): null | Uint8Array;
  print(x: string): void;
}

declare let deno: Deno;
