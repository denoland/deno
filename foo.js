const pattern = new URLPattern("https://example.com/2022/feb/*");

const start = performance.now();
for (let i = 0; i < 1_000_000; i++) {
  pattern.test("https://example.com/2022/feb/xc44rsz");
}
const end = performance.now();
console.log("took", end - start, "ms");
