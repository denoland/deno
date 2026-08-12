// `Deno.writeTextFile` to a file whose parent is an ancestor of the denied
// path must succeed; writes inside the denied scope are still blocked.
Deno.writeTextFileSync("hello.txt", "hi");
console.log("write hello.txt: ok");

try {
  Deno.writeTextFileSync("denied/x.txt", "x");
  console.log("write denied/x.txt: UNEXPECTED OK");
} catch (e) {
  console.log("write denied/x.txt:", (e as Error).name);
}
