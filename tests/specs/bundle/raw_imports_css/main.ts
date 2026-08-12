import style from "./style.css" with { type: "css" };

console.log(style instanceof CSSStyleSheet);
console.log(style.cssRules.length);
console.log(style.cssRules[0].cssText);
