function nonAnalyzablePath() {
  return "./non_analyzable.txt";
}

const { default: helloText } = await import("./hello.txt", { with: { type: "text" } });
const { default: helloBytes } = await import("./hello.txt", { with: { type: "bytes" } });
const { default: nonAnalyzableText } = await import(nonAnalyzablePath(), { with: { type: "text" } });

console.log(helloText);
console.log(helloBytes);
console.log(nonAnalyzableText);
