import * as ts from "typescript";

V8Worker2.recv((ab: ArrayBuffer) {
  V8Worker2.print("Got array buffer", ab.byteLength);
});

V8Worker2.print("Hello");
