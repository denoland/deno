// `localStorage` should be available in a compiled binary and persist across
// runs, keyed by the app identity (no `--location` required).
if (Deno.args[0] === "set") {
  localStorage.setItem("greeting", "hi deno team.");
  console.log("set");
} else {
  console.log("value:", localStorage.getItem("greeting"));
}
