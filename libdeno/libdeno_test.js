// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// A simple runtime that doesn't involve typescript or protobufs to test
// libdeno. Invoked by libdeno_test.cc

const global = this;

function assert(cond) {
  if (!cond) throw Error("libdeno_test.js assert failed");
}

global.CanCallFunction = () => {
  libdeno.print("Hello world from foo");
  return "foo";
};

// This object is created to test snapshotting.
// See DeserializeInternalFieldsCallback and SerializeInternalFieldsCallback.
const snapshotted = new Uint8Array([1, 3, 3, 7]);

global.TypedArraySnapshots = () => {
  assert(snapshotted[0] === 1);
  assert(snapshotted[1] === 3);
  assert(snapshotted[2] === 3);
  assert(snapshotted[3] === 7);
};

global.RecvReturnEmpty = () => {
  const m1 = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const m2 = m1.slice();
  const r1 = libdeno.send(m1);
  assert(r1 == null);
  const r2 = libdeno.send(m2);
  assert(r2 == null);
};

global.RecvReturnBar = () => {
  const m = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const r = libdeno.send(m);
  assert(r instanceof Uint8Array);
  assert(r.byteLength === 3);
  const rstr = String.fromCharCode(...r);
  assert(rstr === "bar");
};

global.DoubleRecvFails = () => {
  // libdeno.recv is an internal function and should only be called once from the
  // runtime.
  libdeno.recv((channel, msg) => assert(false));
  libdeno.recv((channel, msg) => assert(false));
};

global.SendRecvSlice = () => {
  const abLen = 1024;
  let buf = new Uint8Array(abLen);
  for (let i = 0; i < 5; i++) {
    // Set first and last byte, for verification by the native side.
    buf[0] = 100 + i;
    buf[buf.length - 1] = 100 - i;
    // On the native side, the slice is shortened by 19 bytes.
    buf = libdeno.send(buf);
    assert(buf.byteOffset === i * 11);
    assert(buf.byteLength === abLen - i * 30 - 19);
    assert(buf.buffer.byteLength == abLen);
    // Look for values written by the backend.
    assert(buf[0] === 200 + i);
    assert(buf[buf.length - 1] === 200 - i);
    // On the JS side, the start of the slice is moved up by 11 bytes.
    buf = buf.subarray(11);
    assert(buf.byteOffset === (i + 1) * 11);
    assert(buf.byteLength === abLen - (i + 1) * 30);
  }
};

global.JSSendArrayBufferViewTypes = () => {
  // Test that ArrayBufferView slices are transferred correctly.
  // Send Uint8Array.
  const ab1 = new ArrayBuffer(4321);
  const u8 = new Uint8Array(ab1, 2468, 1000);
  u8[0] = 1;
  libdeno.send(u8);
  // Send Uint32Array.
  const ab2 = new ArrayBuffer(4321);
  const u32 = new Uint32Array(ab2, 2468, 1000 / Uint32Array.BYTES_PER_ELEMENT);
  u32[0] = 0x02020202;
  libdeno.send(u32);
  // Send DataView.
  const ab3 = new ArrayBuffer(4321);
  const dv = new DataView(ab3, 2468, 1000);
  dv.setUint8(0, 3);
  libdeno.send(dv);
};

// The following join has caused SnapshotBug to segfault when using kKeep.
[].join("");

global.SnapshotBug = () => {
  assert("1,2,3" === String([1, 2, 3]));
};

global.GlobalErrorHandling = () => {
  eval("\n\n notdefined()\n//# sourceURL=helloworld.js");
};

// Allocate this buf at the top level to avoid GC.
const dataBuf = new Uint8Array([3, 4]);

global.DataBuf = () => {
  const a = new Uint8Array([1, 2]);
  const b = dataBuf;
  // The second parameter of send should modified by the
  // privileged side.
  const r = libdeno.send(a, b);
  assert(r == null);
  // b is different.
  assert(b[0] === 4);
  assert(b[1] === 2);
  // Now we modify it again.
  b[0] = 9;
  b[1] = 8;
};

global.CheckPromiseErrors = () => {
  async function fn() {
    throw new Error("message");
  }

  (async () => {
    try {
      await fn();
    } catch (e) {
      libdeno.send(new Uint8Array([42]));
    }
  })();
};

global.Shared = () => {
  const ab = libdeno.shared;
  assert(ab instanceof SharedArrayBuffer);
  assert(libdeno.shared != undefined);
  assert(ab.byteLength === 3);
  const ui8 = new Uint8Array(ab);
  assert(ui8[0] === 0);
  assert(ui8[1] === 1);
  assert(ui8[2] === 2);
  ui8[0] = 42;
  ui8[1] = 43;
  ui8[2] = 44;
};

global.LibDenoEvalContext = () => {
  const [result, errInfo] = libdeno.evalContext("let a = 1; a");
  assert(result === 1);
  assert(!errInfo);
  const [result2, errInfo2] = libdeno.evalContext("a = a + 1; a");
  assert(result2 === 2);
  assert(!errInfo2);
};

global.LibDenoEvalContextError = () => {
  const [result, errInfo] = libdeno.evalContext("not_a_variable");
  assert(!result);
  assert(!!errInfo);
  assert(errInfo.isNativeError); // is a native error (ReferenceError)
  assert(!errInfo.isCompileError); // is NOT a compilation error
  assert(errInfo.thrown.message === "not_a_variable is not defined");

  const [result2, errInfo2] = libdeno.evalContext("throw 1");
  assert(!result2);
  assert(!!errInfo2);
  assert(!errInfo2.isNativeError); // is NOT a native error
  assert(!errInfo2.isCompileError); // is NOT a compilation error
  assert(errInfo2.thrown === 1);

  const [result3, errInfo3] =
  libdeno.evalContext("class AError extends Error {}; throw new AError('e')");
  assert(!result3);
  assert(!!errInfo3);
  assert(errInfo3.isNativeError); // extend from native error, still native error
  assert(!errInfo3.isCompileError); // is NOT a compilation error
  assert(errInfo3.thrown.message === "e");

  const [result4, errInfo4] = libdeno.evalContext("{");
  assert(!result4);
  assert(!!errInfo4);
  assert(errInfo4.isNativeError); // is a native error (SyntaxError)
  assert(errInfo4.isCompileError); // is a compilation error! (braces not closed)
  assert(errInfo4.thrown.message === "Unexpected end of input");

  const [result5, errInfo5] = libdeno.evalContext("eval('{')");
  assert(!result5);
  assert(!!errInfo5);
  assert(errInfo5.isNativeError); // is a native error (SyntaxError)
  assert(!errInfo5.isCompileError); // is NOT a compilation error! (just eval)
  assert(errInfo5.thrown.message === "Unexpected end of input");
};
