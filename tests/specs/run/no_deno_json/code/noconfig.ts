// ensure the cwd is this directory
const cwd = Deno.cwd();
if (!cwd.endsWith("code")) {
  console.log(cwd);
  throw "FAIL";
} else {
  console.log("success");
}
