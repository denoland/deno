import sheet from "./style.css" with { type: "css" };

const { default: dynamicSheet } = await import("./style.css", {
  with: { "type": "css" },
});

let validSheet: CSSStyleSheet;
validSheet = sheet;
validSheet = dynamicSheet;
validSheet = new CSSStyleSheet();

const rules: readonly CSSRule[] = sheet.cssRules;
const text: string = rules[0].cssText;
sheet.replaceSync(text);
const replaced: Promise<CSSStyleSheet> = sheet.replace("a { b: c }");
console.log(sheet instanceof CSSStyleSheet, replaced);

let invalid: number;
invalid = sheet;
invalid = dynamicSheet;
invalid = sheet.cssRules;
invalid = rules[0].cssText;
sheet.insertRule("a { b: c }");
