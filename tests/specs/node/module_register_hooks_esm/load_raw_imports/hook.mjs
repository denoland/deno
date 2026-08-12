import { registerHooks } from "node:module";

// A passthrough load hook that only delegates via `nextLoad`. Even with no
// transformation, this must not break `type: "bytes"` / `type: "text"`
// imports: the bridge has to fall through to default loading so the module
// keeps its correct (non-string) representation rather than tripping
// "Source code for Bytes module must be provided as bytes".
registerHooks({
  load(url, context, nextLoad) {
    return nextLoad(url, context);
  },
});
