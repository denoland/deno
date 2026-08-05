const outside = Deno.lstatSync("outside");
if (!outside.isDirectory || outside.isSymlink) {
  throw new Error("outside path was replaced");
}

const marker = Deno.readTextFileSync("outside/marker.txt");
if (marker !== "keep\n") {
  throw new Error("outside marker was modified");
}

console.log("preserved");
