// Environment shim for import.meta.env
var __env__ = {
  MODE: "development",
  DEV: true,
  PROD: false,
  SSR: true,
};
if (typeof globalThis !== "undefined") {
  globalThis.__VBUNDLE_ENV__ = __env__;
}

// Module: file:///Users/marvinh/dev/denoland/deno/tests/specs/vbundle/plugins/app.ts
var __module_0__ = (function(exports, module) {
  // Entry point for transform plugin test
  const DEBUG = false;
  const VERSION = "1.0.0";
  if (DEBUG) {
    console.log(`App version: ${VERSION}`);
  }
  export { DEBUG, VERSION };
return module.exports;
})(Object.create(null), { exports: Object.create(null) });


// Entry point
var __entry__ = __module_0__;
