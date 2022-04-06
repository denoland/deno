const sheet = new CSSStyleSheet();
sheet.insertRule("body { background-color: currentColor; }");
console.log(sheet);
const rule = sheet.cssRules.item(0);
console.log(rule);
