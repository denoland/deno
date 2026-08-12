// Verify the barrel/.mts rolled-up .d.ts file content
const path = Deno.args[0];
const content = Deno.readTextFileSync(path);

// Wildcard re-export (export *), .mts re-export, and local const must all be inlined
const expected = ["Leaf", "Settings", "NAME"];
const missing = expected.filter((e) => !content.includes(e));
if (missing.length > 0) {
  console.error("Missing declarations:", missing.join(", "));
  console.error("Content:", content);
  Deno.exit(1);
}

// No relative re-exports/imports should remain in the flattened output
if (/from ['"]\.\.?\//.test(content)) {
  console.error("Found relative imports in flattened .d.ts:");
  console.error(content);
  Deno.exit(1);
}

console.log("ok");
