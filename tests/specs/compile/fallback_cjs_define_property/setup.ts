// Two CJS modules using the `Object.defineProperty(exports, "name", { get })`
// pattern that TypeScript emits when targeting CommonJS, and that npm
// packages like `@opentelemetry/api` ship in their CJS build. `.cjs`
// extension so the runtime classifies them as CJS without needing a
// `package.json` with `"type": "commonjs"`.
Deno.writeTextFileSync(
  "inner.cjs",
  `"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.value = 42;
`,
);
Deno.writeTextFileSync(
  "cjs_pkg.cjs",
  `"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.value = void 0;
const inner_1 = require("./inner.cjs");
Object.defineProperty(exports, "value", {
  enumerable: true,
  get: function () { return inner_1.value; },
});
`,
);
// The entry uses a dynamic import the compile-time graph walker can't
// statically follow — `Function("p", "return import(p)")` evades AST
// analysis cleanly. That keeps `cjs_pkg.cjs` and `inner.cjs` out of
// the embedded VFS, forcing the host-FS fallback paths exercised by
// this regression test.
Deno.writeTextFileSync(
  "main.ts",
  `const dynImport = new Function(
  "p",
  "return import(p);",
) as (p: string) => Promise<{ value: number }>;
const url = new URL("./cjs_pkg.cjs", "file://" + Deno.cwd() + "/").href;
const mod = await dynImport(url);
console.log("ok", mod.value);
`,
);
