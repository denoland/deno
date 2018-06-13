// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
type MessageCallback = (msg: ArrayBuffer) => void;
declare function denoSub(channel: string, cb: MessageCallback): void;
declare function denoPub(channel: string, msg: ArrayBuffer): null | ArrayBuffer;
declare function denoPrint(x: string): void;
