const path = await Deno.makeTempFile({ dir: `subdir` });
try {
  if (!path.match(/^subdir[/\\][^/\\]+/)) {
    throw Error("bad " + path);
  }
  console.log("good", path);
} finally {
  await Deno.remove(path);
}
