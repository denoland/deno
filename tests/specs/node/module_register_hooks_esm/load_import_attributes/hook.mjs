import { registerHooks } from "node:module";
import assert from "node:assert";

registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith("mod.css")) {
      // The load hook must observe the full `with { ... }` clause, not just
      // the `type` attribute. V8 only forwards `type` to the host load
      // callback, so the runtime recovers the rest from the resolve phase.
      assert.deepStrictEqual(context.importAttributes, {
        __proto__: null,
        x_loader: "css-mod",
      });
      // `context.conditions` must match Node's default ESM resolution set.
      assert.deepStrictEqual(context.conditions, [
        "node",
        "import",
        "module-sync",
        "node-addons",
      ]);
      return {
        source: 'export default "css-content";',
        format: "module",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});
