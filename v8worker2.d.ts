// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
declare namespace V8Worker2 {
  function print(...args: any[]): void;
  type RecvCallback = (ab: ArrayBuffer) => void;
  function recv(cb: RecvCallback): void;
  function send(ab: ArrayBuffer): null | ArrayBuffer;
}
