for (const project of ["a", "b"]) {
  Deno.mkdirSync(project);
  Deno.writeTextFileSync(
    `${project}/deno.json`,
    JSON.stringify({
      imports: {
        pkg: "npm:@denotest/esm-package-no-default-export-types@1.0.0",
      },
      nodeModulesDir: "auto",
      unstable: ["npm-global-virtual-store"],
    }),
  );
  Deno.copyFileSync("main.ts", `${project}/main.ts`);
}

for (const project of ["c", "d"]) {
  Deno.mkdirSync(project);
  Deno.writeTextFileSync(
    `${project}/deno.json`,
    JSON.stringify({
      imports: {
        pkg: "npm:@denotest/lifecycle-scripts-counter@1.0.0",
      },
      nodeModulesDir: "auto",
      unstable: ["npm-global-virtual-store"],
      allowScripts: true,
    }),
  );
}

Deno.mkdirSync("e");
Deno.mkdirSync("fake-next");
Deno.writeTextFileSync(
  "fake-next/package.json",
  JSON.stringify({
    name: "next",
    version: "0.0.0",
    main: "main.js",
  }),
);
Deno.writeTextFileSync("fake-next/main.js", 'module.exports = "fake-next";\n');
Deno.writeTextFileSync(
  "e/package.json",
  JSON.stringify({
    dependencies: {
      next: "file:../fake-next",
      "@denotest/esm-package-no-default-export-types": "1.0.0",
    },
    optionalDependencies: {
      vite: "1.0.0",
    },
  }),
);

Deno.mkdirSync("h");
Deno.writeTextFileSync(
  "h/package.json",
  JSON.stringify({
    dependencies: {
      "@denotest/esm-package-no-default-export-types": "1.0.0",
    },
    peerDependencies: {
      vite: "1.0.0",
    },
  }),
);

Deno.mkdirSync("patch-target");
Deno.writeTextFileSync(
  "patch-target/package.json",
  JSON.stringify({
    name: "@denotest/gvs-patch-target",
    version: "1.0.0",
    main: "main.js",
  }),
);
Deno.writeTextFileSync("patch-target/main.js", 'module.exports = "one";\n');

for (const project of ["f", "g"]) {
  Deno.mkdirSync(project);
  Deno.writeTextFileSync(
    `${project}/deno.json`,
    JSON.stringify({
      nodeModulesDir: "auto",
      unstable: ["npm-global-virtual-store"],
      links: ["../patch-target"],
    }),
  );
  Deno.writeTextFileSync(
    `${project}/package.json`,
    JSON.stringify({
      dependencies: {
        "@denotest/gvs-patch-target": "1.0.0",
      },
    }),
  );
}
