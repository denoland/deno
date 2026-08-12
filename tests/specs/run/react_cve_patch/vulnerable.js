// Simulates source patterns from vulnerable react-server-dom-* builds so the
// DENO_PATCH_REACT_CVE mitigation has something to rewrite at load time. The
// `resolved_model` / `fulfilled` markers below mirror the identifiers the
// matcher keys off of, and the surrounding padding mirrors the size of the
// real (minified) snippets the matcher scans.

// CVE-2025-55182: model key parsing without filtering. When the mitigation is
// enabled this `split(":")` gains a `.filter(...)` that drops the dangerous
// "constructor" / "prototype" / "_response" keys.
export function parseModelKeys(str) {
  return str.split(":"); // resolved_model key parsing in the flight client
}

// CVE-2025-55184: a chunk-like thenable that can form a cycle. When the
// mitigation is enabled `then` is wrapped so a self-referential chunk rejects
// instead of looping forever.
export function Chunk(value) {
  this.status = "fulfilled";
  this.value = value;
}
Chunk.prototype.then = function (resolve, reject) {
  // resolved_model fulfilled chunk thenable. The matcher scans ~300 chars
  // after the ".prototype.then" token, mirroring how the real minified
  // react-server-dom builds look, so we keep this body comfortably long with
  // explanatory padding rather than a single tight line of code. This avoids
  // tripping the matcher's length guard while staying behaviourally faithful.
  resolve(this.value);
};
