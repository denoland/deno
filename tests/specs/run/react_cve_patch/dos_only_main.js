import { Chunk } from "./dos_only.js";

// A self-referential chunk rejects when patched, but resolves with itself when
// unpatched. `then` runs synchronously here, so the output is deterministic.
const chunk = new Chunk(null);
chunk.value = chunk;
chunk.then(
  (v) =>
    console.log("then:", v === chunk ? "resolved-cycle" : "resolved-other"),
  (e) => console.log("then:", "rejected", e.message),
);
