import assert from "node:assert/strict";
import { registerHooks } from "node:module";

const custom = Object.assign(new TypeError("load hook rejected"), {
  code: "CUSTOM_LOAD_ERROR",
});
const plain = new RangeError("load hook rejected without a code");
const stackless = Object.assign(new TypeError("load hook rejected"), {
  code: "STACKLESS_LOAD_ERROR",
});
delete stackless.stack;
const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith("hooktest:")) {
      return { url: specifier, format: "module", shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    switch (url) {
      case "hooktest:custom":
        throw custom;
      case "hooktest:plain":
        throw plain;
      case "hooktest:stackless":
        throw stackless;
      case "hooktest:primitive":
        throw "load hook threw a string";
      case "hooktest:null":
        throw null;
      case "hooktest:undefined":
        throw undefined;
      case "hooktest:nested":
        return {
          source: 'import "hooktest:custom";',
          format: "module",
          shortCircuit: true,
        };
      default:
        return nextLoad(url, context);
    }
  },
});

try {
  const cases = [
    ["hooktest:custom", custom],
    ["hooktest:custom", custom],
    ["hooktest:plain", plain],
    ["hooktest:stackless", stackless],
    ["hooktest:primitive", "load hook threw a string"],
    ["hooktest:null", null],
    ["hooktest:undefined", undefined],
    ["hooktest:nested", custom],
  ];
  const outcomes = await Promise.allSettled(
    cases.map(([specifier]) => import(specifier)),
  );
  for (const [i, outcome] of outcomes.entries()) {
    const [specifier, expected] = cases[i];
    assert.equal(outcome.status, "rejected", specifier);
    assert.equal(outcome.reason, expected, specifier);
  }
} finally {
  hook.deregister();
}

await assert.rejects(
  import("./missing_load_error_fixture.mjs"),
  { code: "ERR_MODULE_NOT_FOUND" },
);
console.log("Load hook exceptions preserved");
