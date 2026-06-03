function nonAnalyzablePath() {
  return "./non_analyzable.txt";
}

const { default: helloText } = await import("./hello.txt", {
  with: { type: "text" },
});
const nonAnalyzableTypeText = "text";
const { default: nonAnalyzableText } = await import(nonAnalyzablePath(), {
  with: { type: nonAnalyzableTypeText },
});

console.log(helloText);
console.log(nonAnalyzableText);
