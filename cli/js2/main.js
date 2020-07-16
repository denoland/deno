function bootstrapMainRuntime() {
  Deno.core.print("hello\n");
}

function bootstrapWorkerRuntime() {
  Deno.core.print("hello\n");
}

// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete Object.prototype.__proto__;

Object.defineProperties(globalThis, {
  bootstrap: {
    value: {
      mainRuntime: bootstrapMainRuntime,
      workerRuntime: bootstrapWorkerRuntime,
    },
    configurable: true,
    writable: true,
  },
});
