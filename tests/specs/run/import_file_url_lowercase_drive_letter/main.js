// Mimic node tools (e.g. vite loading svelte.config.js) that build file
// URLs from a lowercased cwd on Windows.
const url = new URL("./app/entry.js", import.meta.url);
const href = url.href.replace(
  /^file:\/\/\/([A-Za-z]):/,
  (_, letter) => `file:///${letter.toLowerCase()}:`,
);
await import(href);
