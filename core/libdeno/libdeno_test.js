// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// A simple runtime that doesn't involve typescript or protobufs to test
// libdeno. Invoked by libdeno_test.cc

const global = this;

function assert(cond) {
  if (!cond) throw Error("libdeno_test.js assert failed");
}

global.CanCallFunction = () => {
  Deno.core.print("Hello world from foo");
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
  const r1 = Deno.core.send(m1);
  assert(r1 == null);
  const r2 = Deno.core.send(m2);
  assert(r2 == null);
};

global.RecvReturnBar = () => {
  const m = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const r = Deno.core.send(m);
  assert(r instanceof Uint8Array);
  assert(r.byteLength === 3);
  const rstr = String.fromCharCode(...r);
  assert(rstr === "bar");
};

global.DoubleRecvFails = () => {
  // Deno.core.recv is an internal function and should only be called once from the
  // runtime.
  Deno.core.recv((_channel, _msg) => assert(false));
  Deno.core.recv((_channel, _msg) => assert(false));
};

global.SendRecvSlice = () => {
  const abLen = 1024;
  let buf = new Uint8Array(abLen);
  for (let i = 0; i < 5; i++) {
    // Set first and last byte, for verification by the native side.
    buf[0] = 100 + i;
    buf[buf.length - 1] = 100 - i;
    // On the native side, the slice is shortened by 19 bytes.
    buf = Deno.core.send(buf);
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
  Deno.core.send(u8);
  // Send Uint32Array.
  const ab2 = new ArrayBuffer(4321);
  const u32 = new Uint32Array(ab2, 2468, 1000 / Uint32Array.BYTES_PER_ELEMENT);
  u32[0] = 0x02020202;
  Deno.core.send(u32);
  // Send DataView.
  const ab3 = new ArrayBuffer(4321);
  const dv = new DataView(ab3, 2468, 1000);
  dv.setUint8(0, 3);
  Deno.core.send(dv);
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
const zeroCopyBuf = new Uint8Array([3, 4]);

global.ZeroCopyBuf = () => {
  const a = new Uint8Array([1, 2]);
  const b = zeroCopyBuf;
  // The second parameter of send should modified by the
  // privileged side.
  const r = Deno.core.send(a, b);
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
      Deno.core.send(new Uint8Array([42]));
    }
  })();
};

global.Shared = () => {
  const ab = Deno.core.shared;
  assert(ab instanceof SharedArrayBuffer);
  assert(Deno.core.shared != undefined);
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
  const [result, errInfo] = Deno.core.evalContext("let a = 1; a");
  assert(result === 1);
  assert(!errInfo);
  const [result2, errInfo2] = Deno.core.evalContext("a = a + 1; a");
  assert(result2 === 2);
  assert(!errInfo2);
};

global.LibDenoEvalContextError = () => {
  const [result, errInfo] = Deno.core.evalContext("not_a_variable");
  assert(!result);
  assert(!!errInfo);
  assert(errInfo.isNativeError); // is a native error (ReferenceError)
  assert(!errInfo.isCompileError); // is NOT a compilation error
  assert(errInfo.thrown.message === "not_a_variable is not defined");

  const [result2, errInfo2] = Deno.core.evalContext("throw 1");
  assert(!result2);
  assert(!!errInfo2);
  assert(!errInfo2.isNativeError); // is NOT a native error
  assert(!errInfo2.isCompileError); // is NOT a compilation error
  assert(errInfo2.thrown === 1);

  const [result3, errInfo3] = Deno.core.evalContext(
    "class AError extends Error {}; throw new AError('e')"
  );
  assert(!result3);
  assert(!!errInfo3);
  assert(errInfo3.isNativeError); // extend from native error, still native error
  assert(!errInfo3.isCompileError); // is NOT a compilation error
  assert(errInfo3.thrown.message === "e");

  const [result4, errInfo4] = Deno.core.evalContext("{");
  assert(!result4);
  assert(!!errInfo4);
  assert(errInfo4.isNativeError); // is a native error (SyntaxError)
  assert(errInfo4.isCompileError); // is a compilation error! (braces not closed)
  assert(errInfo4.thrown.message === "Unexpected end of input");

  const [result5, errInfo5] = Deno.core.evalContext("eval('{')");
  assert(!result5);
  assert(!!errInfo5);
  assert(errInfo5.isNativeError); // is a native error (SyntaxError)
  assert(!errInfo5.isCompileError); // is NOT a compilation error! (just eval)
  assert(errInfo5.thrown.message === "Unexpected end of input");
};

global.LibDenoGCObserver = () => {
  // Basic usage.
  let num = 1;
  const incr = () => {
    Deno.core.print("Destroy callback invoked\n");
    num = num + 1;
  };
  // IIFE. Simply using scope might not work.
  (() => {
    const o = Deno.core.setGCObserver({}, incr);
    assert(!!o);
  })();
  // Guarantee GC.
  gc();
  assert(num === 2);

  // Accessing the gc-ed object in callback.
  let num2 = 0;
  // IIFE. Simply using scope might not work.
  (() => {
    const obj = { a: 10 };
    const incr2 = o => {
      Deno.core.print("Destroy callback invoked 2\n");
      num2 = o.a;
    };
    Deno.core.setGCObserver(obj, incr2);
  })();
  // Guarantee GC.
  gc();
  assert(num2 === 10);

  // Simulating a connection goes out of scope
  // for auto closing.
  let isClosed = false;
  let ID = 12345;
  const fakeClose = id => {
    if (id === ID) {
      isClosed = true;
    }
  };
  class FakeConn {
    constructor(rid) {
      this.rid = rid;
    }
    close() {
      Deno.core.print("Closing fake connection\n");
      fakeClose(this.rid);
    }
  }
  // IIFE. Simply using scope might not work.
  (() => {
    const conn = new FakeConn(12345);
    Deno.core.setGCObserver(conn, c => {
      Deno.core.print("Destroy callback invoked 3\n");
      c.close();
    });
  })();
  // Guarantee GC.
  gc();
  assert(isClosed);
};
