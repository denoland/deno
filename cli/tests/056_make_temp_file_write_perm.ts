const path = await Deno.makeTempFile({ dir: "./subdir/" });
if (path.startsWith(Deno.cwd())) {
  console.log("good", path);
} else {
  throw Error("bad " + path);
}
console.log(path);
await Deno.remove(path);
