console.log(typeof Deno.dlopen === "function"); // Undefined without `--unstable-ffi`
console.log(
  // Undefined without `--unstable-fs`
  Deno.build.os === "windows" ? true : typeof Deno.umask === "function",
);
