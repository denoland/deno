console.log(
  // Undefined without `--unstable-fs`
  Deno.build.os === "windows" ? true : typeof Deno.umask === "function",
);
