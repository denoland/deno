import { register } from "node:module";

// Reproduces the regression hit by lume's MDX plugin when any registered
// resolve hook (e.g. @tailwindcss/postcss transitive) flips on
// `resolve_active`. The MDX plugin compiles MDX to a `blob:` module that
// statically imports bare-mapped specifiers like `lume/jsx-runtime`. The
// blob URL is `cannot-be-a-base`, so the async-resolve placeholder used
// to error with `relative URL with a cannot-be-a-base base` before the
// hook chain had a chance to run.
register("./hooks.mjs", import.meta.url);

const code = `import { value } from "@mapped/foo";\n` +
  `console.log("blob got:", value);\n`;
const blob = new Blob([code], { type: "text/javascript" });
const url = URL.createObjectURL(blob);
await import(url);
console.log("done");
