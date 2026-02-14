const file = Deno.args[0];

const contents = Deno.readTextFileSync(file);

if (contents.includes("createRequire")) {
  console.log("true");
} else {
  console.log("false");
}
