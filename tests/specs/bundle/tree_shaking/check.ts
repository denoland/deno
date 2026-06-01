const bundle = await Deno.readTextFile("out.js");
if (!bundle.includes("USED_MARKER")) {
  console.log("missing used export");
  Deno.exit(1);
}
if (bundle.includes("UNUSED_MARKER")) {
  console.log("unused export was not tree-shaken");
  Deno.exit(1);
}
console.log("ok");
