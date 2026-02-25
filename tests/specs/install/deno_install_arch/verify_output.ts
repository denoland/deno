const metadata = Deno.statSync(
  "node_modules/.deno/@denotest+multiple-arches-win32-arm64@1.0.0",
);
if (metadata) {
  console.log("FOUND");
}
