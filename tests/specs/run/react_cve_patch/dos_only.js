// CVE-2025-55184 in isolation: a chunk-like thenable that can form a cycle,
// with no `split(...)` model-key pattern anywhere in the module, so only the
// stage 2 (DoS) patch should apply.
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
