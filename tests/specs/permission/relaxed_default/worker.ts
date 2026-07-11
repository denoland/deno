// Inherits the relaxed profile from the parent: read is confined to the cwd and
// temp directory, env is the curated allowlist, and net stays gated. Reading a
// file inside the cwd succeeds; reading outside the cwd and temp is denied.
let read;
try {
  Deno.readTextFileSync("./worker.ts");
  read = "ok";
} catch (err) {
  read = err.name;
}
let readOutside;
try {
  Deno.readFileSync(Deno.execPath());
  readOutside = "UNEXPECTEDLY ALLOWED";
} catch (err) {
  readOutside = err.name;
}
let env;
try {
  Deno.env.get("TERM");
  env = "ok";
} catch (err) {
  env = err.name;
}
let net;
try {
  await fetch("http://localhost/");
  net = "ok";
} catch (err) {
  net = err.name;
}
self.postMessage(
  `worker read: ${read}, worker read outside: ${readOutside}, ` +
    `worker env: ${env}, worker fetch: ${net}`,
);
