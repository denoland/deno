import render from "data:text/jsx;base64,ZXhwb3J0IGRlZmF1bHQgZnVuY3Rpb24oKSB7CiAgcmV0dXJuIDxkaXY+SGVsbG8gRGVubyE8L2Rpdj4KfQo=";

// deno-lint-ignore no-explicit-any
(globalThis as any).React = {
  createElement(...args: unknown[]) {
    console.log(...args);
  },
};

render();
