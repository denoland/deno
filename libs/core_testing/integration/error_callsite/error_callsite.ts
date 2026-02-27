// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any
function toObj(callsite: any) {
  const keys = [
    "getThis",
    "getTypeName",
    "getFunction",
    "getFunctionName",
    "getMethodName",
    "getFileName",
    "getLineNumber",
    "getColumnNumber",
    "getEvalOrigin",
    "isToplevel",
    "isEval",
    "isNative",
    "isConstructor",
    "isAsync",
    "isPromiseAll",
    "getPromiseIndex",
  ];
  return Object.fromEntries(keys.map((key) => [key, callsite[key]()]));
}
(Error as any).prepareStackTrace = function (_: any, callsites: any) {
  callsites.forEach((callsite: any) => {
    console.log(toObj(callsite));
    console.log(callsite.toString());
  });
  return callsites;
};

class Foo {
  constructor() {
    new Error().stack;
  }
}

new Foo();
