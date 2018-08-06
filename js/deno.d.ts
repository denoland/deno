// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Prototype https://github.com/denoland/deno/blob/golang/deno.d.ts

declare module "deno" {
  type MessageCallback = (msg: Uint8Array) => void;

  function recv(cb: MessageCallback): void;
  function send(msg: ArrayBufferView): null | Uint8Array;
  function print(x: string): void;
  function readFileSync(filename: string): Uint8Array;
}
