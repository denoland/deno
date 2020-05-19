const sep = Deno.build.os == "windows" ? "\\" : "/";
const path = await Deno.makeTempFile({ dir: `.${sep}subdir` });
if (path.startsWith(`.${sep}subdir${sep}`)) {
  console.log("good", path);
} else {
  throw Error("bad " + path);
}
console.log(path);
await Deno.remove(path);
