import { Chunk, parseModelKeys } from "./vulnerable.js";

// RCE: dangerous keys are stripped only when the mitigation is enabled.
console.log("keys:", JSON.stringify(parseModelKeys("a:constructor:b")));

// DoS: a self-referential chunk rejects when patched, but resolves with
// itself when unpatched. `then` runs synchronously here, so the output order
// is deterministic.
const chunk = new Chunk(null);
chunk.value = chunk;
chunk.then(
  (v) =>
    console.log("then:", v === chunk ? "resolved-cycle" : "resolved-other"),
  (e) => console.log("then:", "rejected", e.message),
);
