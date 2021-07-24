const resp = await fetch("http://localhost:4545/single-large-file");
const buff = await resp.arrayBuffer();
if (buff.byteLength !== 100_000_000) {
  throw new Error("Downloaded file size is not the same from original");
}
Deno.exit(0);
