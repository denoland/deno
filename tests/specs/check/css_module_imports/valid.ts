import sheet from "./style.css" with { type: "css" };

const rules: readonly CSSRule[] = sheet.cssRules;
console.log(sheet instanceof CSSStyleSheet, rules.length);
