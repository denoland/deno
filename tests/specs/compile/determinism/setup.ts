// for setup, we create two directories with the same file in each
// and then when compiling we ensure this directory name has no
// effect on the output
makeCopyDir("a");
makeCopyDir("b");

function makeCopyDir(dirName) {
  Deno.mkdirSync(dirName);
  Deno.copyFileSync("main.ts", `${dirName}/main.ts`);
}
