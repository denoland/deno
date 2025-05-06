const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libFileName = `${libPrefix}test_ffi.${libSuffix}`;
const libPath = `${targetDir}/${libFileName}`;

Deno.copyFileSync(libPath, import.meta.dirname + "/" + libFileName);
