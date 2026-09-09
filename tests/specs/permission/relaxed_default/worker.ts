// Inherits the relaxed profile from the parent: read is allowed everywhere but
// net stays gated.
let read;
try {
  Deno.readFileSync(Deno.execPath());
  read = "ok";
} catch (err) {
  read = err.name;
}
let net;
try {
  await fetch("http://localhost/");
  net = "ok";
} catch (err) {
  net = err.name;
}
self.postMessage(`worker read: ${read}, worker fetch: ${net}`);
