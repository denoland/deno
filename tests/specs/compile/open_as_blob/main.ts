import { openAsBlob } from "node:fs";

const blob = await openAsBlob(new URL("./data.txt", import.meta.url), {
  type: "text/plain",
});
console.log(blob.size, blob.type, await blob.slice(2, 5).text());
const url = URL.createObjectURL(new Blob(["prefix:", blob.slice(0, 10)]));
try {
  console.log(await (await fetch(url)).text());
} finally {
  URL.revokeObjectURL(url);
}
