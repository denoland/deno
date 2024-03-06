// ensure the cwd is this directory
const cwd = Deno.cwd();
if (!cwd.endsWith("no_deno_json")) {
  console.log(cwd);
  throw "FAIL";
} else {
  console.log("success");
}
