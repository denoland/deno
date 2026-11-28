import assert from "node:assert/strict";
import { registerHooks } from "node:module";

const resolved = [];
const loaded = [];
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "./mod.css") {
      resolved.push({ ...context.importAttributes });
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (!url.endsWith("/mod.css")) return nextLoad(url, context);
    loaded.push({ ...context.importAttributes });
    // An empty object is valid source for both module types.
    return {
      format: context.importAttributes.type ?? "module",
      source: "{}",
      shortCircuit: true,
    };
  },
});

const fr = { locale: "fr" };
const it = { locale: "it" };
const jsonEn = { type: "json", locale: "en" };
const jsonDe = { type: "json", locale: "de" };
const load = (attributes) => import("./mod.css", { with: attributes });

// Each URL and type is loaded with the attributes of its first request.
await Promise.all([import("./mod.css"), load(fr)]);
await load(it); // Cache hit.
await Promise.all([load(jsonEn), load(jsonDe)]);

assert.deepEqual(resolved, [{}, fr, it, jsonEn, jsonDe]);
assert.deepEqual(loaded, [{}, jsonEn]);
