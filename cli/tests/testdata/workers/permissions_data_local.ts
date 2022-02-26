// This file doesn't really exist, but it doesn't matter, a "PermissionsDenied" error should be thrown.
const code = `import "file:///${
  Deno.build.os == "windows" ? "C:/" : ""
}local_file.ts";`;
new Worker(`data:application/javascript;base64,${btoa(code)}`, {
  type: "module",
});
