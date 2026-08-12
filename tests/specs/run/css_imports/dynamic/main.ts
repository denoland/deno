function nonAnalyzablePath() {
  return "./other.css";
}

const { default: sheet } = await import("./style.css", {
  with: { type: "css" },
});
console.log(sheet instanceof CSSStyleSheet);
console.log(sheet.cssRules[0].cssText);

const { default: other } = await import(nonAnalyzablePath(), {
  with: { type: "css" },
});
console.log(other.cssRules[0].cssText);

const constructed = new CSSStyleSheet();
await constructed.replace(".a { color: blue; }");
console.log(constructed.cssRules[0].cssText);

// Constructed sheets disallow `@import`, so `replace()`/`replaceSync()` drop
// any top-level `@import` rules.
constructed.replaceSync('@import "ignored.css"; .c { color: red; }');
console.log(constructed.cssRules.length, constructed.cssRules[0].cssText);

constructed.replaceSync("");
console.log(constructed.cssRules.length);
