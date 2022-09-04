// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// https://github.com/facebook/react/blob/f0efa1164b7ca8523b081223954d05c88e92053b/packages/react-dom/src/server/escapeTextForBrowser.js#L51
const matchHtmlRegExp = /["'&<>]/;
function escapeHtmlSmol(string) {
  const str = "" + string;
  const match = matchHtmlRegExp.exec(str);

  if (!match) {
    return str;
  }

  let escape;
  let html = "";
  let index;
  let lastIndex = 0;

  for (index = match.index; index < str.length; index++) {
    switch (str.charCodeAt(index)) {
      case 34: // "
        escape = "&quot;";
        break;
      case 38: // &
        escape = "&amp;";
        break;
      case 39: // '
        escape = "&#x27;"; // modified from escape-html; used to be '&#39'
        break;
      case 60: // <
        escape = "&lt;";
        break;
      case 62: // >
        escape = "&gt;";
        break;
      default:
        continue;
    }

    if (lastIndex !== index) {
      html += str.substring(lastIndex, index);
    }

    lastIndex = index + 1;
    html += escape;
  }

  return lastIndex !== index ? html + str.substring(lastIndex, index) : html;
}

const labels = [];
const jsValues = [];
const nativeValues = [];

console.log("%crunning Deno.escapeHTML benchmark", "color: green");

let i = 10;
while (true) {
  const t = "<h1>".repeat(i);
  labels.push(t.length);
  const start = performance.now();
  for (let k = 0; k < 100; k++) {
    Deno.escapeHtml(t);
  }
  const elapsed = performance.now() - start;
  nativeValues.push(elapsed);
  if (t.length > 100 * 1024) {
    break;
  }

  i += 100;
}

console.log(
  "%cavg time:",
  "color: green",
  nativeValues.reduce((a, b) => a + b) / nativeValues.length,
  "\n",
);

console.log("%crunning react-dom escapeHTML benchmark", "color: green");

i = 10;
while (true) {
  const t = "<h1>".repeat(i);
  const start2 = performance.now();
  for (let k = 0; k < 100; k++) {
    escapeHtmlSmol(t);
  }
  const elapsed2 = performance.now() - start2;
  jsValues.push(elapsed2);
  if (t.length > 100 * 1024) {
    break;
  }

  i += 100;
}

console.log(
  "%cavg time:",
  "color: green",
  jsValues.reduce((a, b) => a + b) / jsValues.length,
  "\n",
);

Deno.writeTextFileSync(
  new URL("results.json", import.meta.url),
  JSON.stringify({
    labels,
    jsValues,
    nativeValues,
  }),
);

const { serve } = await import("./serve_chart.jsx");
console.log(
  "%cresults written to results.json. view rendered report at http://localhost:9000/",
  "color: yellow",
);
serve();
