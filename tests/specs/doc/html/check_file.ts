const content = Deno.readTextFileSync("./docs/index.html");

if (content.includes("..")) {
  throw new Error();
}
