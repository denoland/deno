const dataSize = 1024 * 1024 * 8;
const fileName = await Deno.makeTempFile();
const file = await Deno.open(fileName, { read: true, write: true });
const dataBuf = new Uint8Array(dataSize);
dataBuf.fill(65);
await Deno.writeAll(file, dataBuf);

const promises = [];
for (let i = 0; i < 8; ++i) {
  promises.push(Deno.readTextFile(fileName));
}

await Promise.all(promises);
console.log("Reads Complete");
await Deno.close(file.rid);
await Deno.remove(fileName);
