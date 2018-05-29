// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
declare module "deno" {
  type MessageCallback = (msg: Uint8Array) => void;
  function sub(channel: string, cb: MessageCallback): void;
  function pub(channel: string, payload: Uint8Array): null | ArrayBuffer;

  function readFileSync(filename: string): Uint8Array;
  function writeFileSync(
    filename: string,
    data: Uint8Array,
    perm: number
  ): void;
}
