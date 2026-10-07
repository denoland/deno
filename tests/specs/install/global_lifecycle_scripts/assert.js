const script = Deno.readTextFileSync(
  "./bins/bin/.testbin/node_modules/@denotest/bin-created-by-lifecycle/testbin.js",
);

if (!script.includes("run testbin")) {
  throw new Error("install script did not create the bin entry");
}
