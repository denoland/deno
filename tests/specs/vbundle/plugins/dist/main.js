// Environment shim for import.meta.env
var __env__ = {
  MODE: "development",
  DEV: true,
  PROD: false,
  SSR: false,
};
if (typeof globalThis !== "undefined") {
  globalThis.__VBUNDLE_ENV__ = __env__;
}

// Module: file:///Users/marvinh/dev/denoland/deno/tests/specs/vbundle/plugins/main.ts
var __module_0__ = (function(exports, module) {
  // Simple entry point for basic vbundle test
  export function greet(name) {
    return `Hello, ${name}!`;
  }
  console.log(greet("World"));
return module.exports;
})(Object.create(null), { exports: Object.create(null) });


// Entry point
var __entry__ = __module_0__;
