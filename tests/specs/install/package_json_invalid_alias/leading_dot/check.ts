const dotDeno = Deno.lstatSync("node_modules/.deno");
if (!dotDeno.isDirectory || dotDeno.isSymlink) {
  throw new Error("node_modules/.deno was replaced");
}

const marker = Deno.readTextFileSync("node_modules/.deno/marker.txt");
if (marker !== "keep\n") {
  throw new Error("node_modules/.deno marker was modified");
}

console.log("preserved");
