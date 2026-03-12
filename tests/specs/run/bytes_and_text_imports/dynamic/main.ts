function nonAnalyzablePath() {
  return "./non_analyzable.txt";
}

function nonAnalyzableUtf8BomPath() {
  return "./non_analyzable_utf8_bom.txt";
}

const { default: helloText } = await import("./hello.txt", {
  with: { type: "text" },
});
const { default: helloBytes } = await import("./hello.txt", {
  with: { type: "bytes" },
});
const nonAnalyzableTypeText = "text";
const { default: nonAnalyzableText } = await import(nonAnalyzablePath(), {
  with: { type: nonAnalyzableTypeText },
});
const { default: utf8BomText } = await import("./utf8_bom.txt", {
  with: { type: "text" },
});
const { default: utf8BomBytes } = await import("./utf8_bom.txt", {
  with: { type: "bytes" },
});
const { default: nonAnalyzableUtf8BomText } = await import(
  nonAnalyzableUtf8BomPath(),
  { with: { type: "text" } }
);
const { default: nonAnalyzableUtf8BomBytes } = await import(
  nonAnalyzableUtf8BomPath(),
  { with: { type: "bytes" } }
);

console.log(helloText);
console.log(helloBytes);
console.log(nonAnalyzableText);
console.log("utf8 bom");
console.log(utf8BomText, utf8BomText.length);
console.log(utf8BomBytes);
console.log("utf8 bom non-analyzable");
console.log(nonAnalyzableUtf8BomText, nonAnalyzableUtf8BomText.length);
console.log(nonAnalyzableUtf8BomBytes);
