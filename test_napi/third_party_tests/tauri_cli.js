const tauri = Deno.core.dlopen(
  "node_modules/@tauri-apps/cli-darwin-arm64/cli.darwin-arm64.node"
);
console.log(tauri.run([], "null", console.log));