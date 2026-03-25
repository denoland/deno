// Sets up node_modules/.deno/ with two versions of @denotest/add
// to test that exact version specifiers are resolved correctly.

const dir = Deno.cwd();

// Create node_modules/.deno/@denotest+add@1.0.0/node_modules/@denotest/add/
const v1Path =
  `${dir}/node_modules/.deno/@denotest+add@1.0.0/node_modules/@denotest/add`;
await Deno.mkdir(v1Path, { recursive: true });
await Deno.writeTextFile(
  `${v1Path}/package.json`,
  JSON.stringify({ name: "@denotest/add", version: "1.0.0" }),
);
await Deno.writeTextFile(
  `${v1Path}/index.js`,
  'module.exports.add = (a, b) => a + b;\nmodule.exports.version = "1.0.0";\n',
);

// Create node_modules/.deno/@denotest+add@0.5.0/node_modules/@denotest/add/
const v05Path =
  `${dir}/node_modules/.deno/@denotest+add@0.5.0/node_modules/@denotest/add`;
await Deno.mkdir(v05Path, { recursive: true });
await Deno.writeTextFile(
  `${v05Path}/package.json`,
  JSON.stringify({ name: "@denotest/add", version: "0.5.0" }),
);
await Deno.writeTextFile(
  `${v05Path}/index.js`,
  'module.exports.sum = (a, b) => a + b;\nmodule.exports.version = "0.5.0";\n',
);

// Create the top-level node_modules/@denotest/add symlink pointing to 1.0.0
// (simulates what `deno install` would create for the latest version)
const topPath = `${dir}/node_modules/@denotest/add`;
await Deno.mkdir(`${dir}/node_modules/@denotest`, { recursive: true });
try {
  await Deno.remove(topPath, { recursive: true });
} catch { /* ignore */ }
await Deno.symlink(
  `${dir}/node_modules/.deno/@denotest+add@1.0.0/node_modules/@denotest/add`,
  topPath,
);

console.log("setup done");
