// Child used by the #35441 regression test. Reading an env var requires
// --allow-env; when spawned as `deno run <script>` (deno-style args) without
// permission flags, the Node-compat translation must still grant full
// permissions, so this should print "env-ok" instead of throwing.
let result;
try {
  void process.env.PATH;
  result = "env-ok";
} catch (e) {
  result = "env-denied: " + e.name;
}
console.log(result);
