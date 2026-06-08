// Verify that chalk is a real directory (hoisted) and not a symlink
const stat = Deno.lstatSync("node_modules/chalk");
console.log("hoisted:", stat.isDirectory && !stat.isSymlink);
