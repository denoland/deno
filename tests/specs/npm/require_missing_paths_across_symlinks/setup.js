const root = Deno.cwd();
const nodeModules = `${root}/node_modules`;

Deno.mkdirSync(`${nodeModules}/regular`, { recursive: true });
Deno.mkdirSync(`${nodeModules}/regular/empty`, { recursive: true });
Deno.mkdirSync(`${nodeModules}/contained-target`, { recursive: true });
Deno.mkdirSync(`${nodeModules}/contained-target/empty`, { recursive: true });
Deno.mkdirSync(`${nodeModules}/contained-removed`, { recursive: true });
Deno.mkdirSync(`${root}/outside`, { recursive: true });
Deno.mkdirSync(`${root}/removed`, { recursive: true });

Deno.writeTextFileSync(
  `${nodeModules}/regular/found.js`,
  "module.exports = 'regular';\n",
);
Deno.writeTextFileSync(
  `${nodeModules}/contained-target/found.js`,
  "module.exports = 'contained';\n",
);
Deno.writeTextFileSync(
  `${root}/outside/found.js`,
  "module.exports = 'outside';\n",
);

const linkType = Deno.build.os === "windows" ? "junction" : "dir";
Deno.symlinkSync(
  `${nodeModules}/contained-target`,
  `${nodeModules}/contained-link`,
  { type: linkType },
);
Deno.symlinkSync(`${root}/outside`, `${nodeModules}/outside-link`, {
  type: linkType,
});
Deno.symlinkSync(`${root}/removed`, `${nodeModules}/dangling-link`, {
  type: linkType,
});
Deno.symlinkSync(
  `${nodeModules}/contained-removed`,
  `${nodeModules}/contained-dangling-link`,
  { type: linkType },
);
Deno.removeSync(`${root}/removed`, { recursive: true });
Deno.removeSync(`${nodeModules}/contained-removed`, { recursive: true });
