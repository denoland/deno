import sheet from "./style.css" with { type: "css" };

console.log(sheet instanceof CSSStyleSheet);
console.log(sheet.cssRules.length);
for (const rule of sheet.cssRules) {
  console.log(rule.cssText);
}
