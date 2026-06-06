// A CJS npm package with `__esModule: true` and a `default` export. Under
// `--bundle` the default import resolves to the whole `module.exports` object,
// exactly like `deno run` (Node interop semantics), so the callable lives on
// `.default`. This locks in that CJS default-export interop works in a
// compiled bundle.
import cjsDefault, { named } from "npm:@denotest/cjs-default-export";

console.log("cjs default", cjsDefault.default());
console.log("cjs named", named());
console.log("cjs class", cjsDefault.MyClass.someStaticMethod());
