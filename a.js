console.time("warmup");
console.timeEnd("warmup");
const data = `{'hello': 123}`.repeat(999_999); //new Array(99_999).fill(`{'hello': 123}`).join("");
const Blob = globalThis.Blob || (await import("buffer")).Blob;
// warmup
for (let i = 0; i < 99999; i++) {
  const b = new Blob(["123"]);
  await b.arrayBuffer();
  await b.text();
}

console.time("new Blob([`{'hello': 123}`.repeat(999_999)])");
const blob = new Blob([data]);
console.timeEnd("new Blob([`{'hello': 123}`.repeat(999_999)])");
console.time("blob.text()");
const text = await blob.text();
console.timeEnd("blob.text()");
console.time("blob.arrayBuffer()");
const arrayBuffer = await blob.arrayBuffer();
console.timeEnd("blob.arrayBuffer()");
