// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const queueMicrotask = globalThis.queueMicrotask || process.nextTick;
let [total, count] = typeof Deno !== "undefined"
  ? Deno.args
  : [process.argv[2], process.argv[3]];

total = total ? parseInt(total, 0) : 50;
count = count ? parseInt(count, 10) : 100000;

function bench(fun) {
  const start = Date.now();
  for (let i = 0; i < count; i++) fun();
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);
  if (--total) queueMicrotask(() => bench(fun));
}

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

const { ops } = Deno.core;
function escapeHtml(str) {
  if (str.length <= 100) {
    return escapeHtmlSmol(str);
  }
  return ops.op_escape_html(str);
}

const t0 = "1".repeat(100);
const labels = [];
const jsValues = [];
const nativeValues = [];

let i = 10;
while (true) {
  const t = "<h1>".repeat(i);
  labels.push(t.length);
  const start = performance.now();
  for (let k = 0; k < 100; k++) {
    ops.op_escape_html(t);
  }
  const elapsed = performance.now() - start;

  const start2 = performance.now();
  for (let k = 0; k < 100; k++) {
    escapeHtmlSmol(t);
  }
  const elapsed2 = performance.now() - start2;
  nativeValues.push(elapsed);
  jsValues.push(elapsed2);

  // 1mb
  if (t.length > 100 * 1024) {
    break;
  }

  i += 100;
}
Deno.writeTextFileSync("results.json", JSON.stringify({
    labels,
    jsValues,
    nativeValues,
}));