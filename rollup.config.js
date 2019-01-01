// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// @ts-check
import * as fs from "fs";
import path from "path";
import alias from "rollup-plugin-alias";
import { plugin as analyze } from "rollup-plugin-analyzer";
import commonjs from "rollup-plugin-commonjs";
import globals from "rollup-plugin-node-globals";
import nodeResolve from "rollup-plugin-node-resolve";
import typescriptPlugin from "rollup-plugin-typescript2";
import { createFilter } from "rollup-pluginutils";
import typescript from "typescript";
import MagicString from "magic-string";

const mockPath = path.resolve(__dirname, "js/mock_builtin.js");
const platformPath = path.resolve(__dirname, "js/platform.ts");
const tsconfig = path.resolve(__dirname, "tsconfig.json");
const typescriptPath = path.resolve(
  __dirname,
  "third_party/node_modules/typescript/lib/typescript.js"
);

// We will allow generated modules to be resolvable by TypeScript based on
// the current build path
const tsconfigOverride = {
  compilerOptions: {
    paths: {
      "*": ["*", path.resolve("*")]
    }
  }
};

/** this is a rollup plugin which will look for imports ending with `!string` and resolve
 * them with a module that will inline the contents of the file as a string.  Needed to
 * support `js/assets.ts`.
 * @param {any} param0
 */
function strings(
  { include, exclude } = { include: undefined, exclude: undefined }
) {
  if (!include) {
    throw new Error("include option must be passed");
  }

  const filter = createFilter(include, exclude);

  return {
    name: "strings",

    /**
     * @param {string} importee
     */
    resolveId(importee) {
      if (importee.endsWith("!string")) {
        // strip the `!string` from `importee`
        importee = importee.slice(0, importee.lastIndexOf("!string"));
        if (!importee.startsWith("gen/")) {
          // this is a static asset which is located relative to the root of
          // the source project
          return path.resolve(path.join(__dirname, importee));
        }
        // this is an asset which has been generated, therefore it will be
        // located within the build path
        return path.resolve(path.join(process.cwd(), importee));
      }
    },

    /**
     * @param {any} code
     * @param {string} id
     */
    transform(code, id) {
      if (filter(id)) {
        return {
          code: `export default ${JSON.stringify(code)};`,
          map: { mappings: "" }
        };
      }
    }
  };
}

const archNodeToDeno = {
  x64: "x64"
};
const osNodeToDeno = {
  win32: "win",
  darwin: "mac",
  linux: "linux"
};

/** Inject deno.platform.arch and deno.platform.os
 * @param {any} param0
 */
function platform({ include, exclude } = {}) {
  if (!include) {
    throw new Error("include option must be passed");
  }

  const filter = createFilter(include, exclude);

  return {
    name: "platform",
    /**
     * @param {any} _code
     * @param {string} id
     */
    transform(_code, id) {
      if (filter(id)) {
        // Adapted from https://github.com/rollup/rollup-plugin-inject/blob/master/src/index.js
        const arch = archNodeToDeno[process.arch];
        const os = osNodeToDeno[process.platform];
        // We do not have to worry about the interface here, because this is just to generate
        // the actual runtime code, not any type information integrated into Deno
        const magicString = new MagicString(`
export const platform = { arch: "${arch}", os:"${os}" };`);
        return {
          code: magicString.toString(),
          map: magicString.generateMap()
        };
      }
    }
  };
}

// This plugin resolves at bundle time any generated resources that are
// in the build path under `gen` and specified with a MID starting with `gen/`.
// The plugin assumes that the MID needs to have the `.ts` extension appended.
function resolveGenerated() {
  return {
    name: "resolve-msg-generated",
    resolveId(importee) {
      if (importee.startsWith("gen/msg_generated")) {
        return path.resolve(`${importee}.ts`);
      }
    }
  };
}

function generateDepFile({ outputFile, sourceFiles = [], configFiles = [] }) {
  let timestamp = new Date();

  // Save the depfile just before the node process exits.
  process.once("beforeExit", () =>
    writeDepFile({ outputFile, sourceFiles, configFiles, timestamp })
  );

  return {
    name: "depfile",
    load(sourceFile) {
      // The 'globals' plugin adds generated files that don't exist on disk.
      // Don't add them to the depfile.
      if (/^[0-9a-f]{30}$/.test(sourceFile)) {
        return;
      }
      sourceFiles.push(sourceFile);
      // Remember the time stamp that we last resolved a dependency.
      // We'll set the last modified time of the depfile to that.
      timestamp = new Date();
    }
  };
}

function writeDepFile({ outputFile, sourceFiles, configFiles, timestamp }) {
  const buildDir = process.cwd();
  const outputDir = path.dirname(outputFile);

  // Assert that the discovered bundle inputs are files that exist on disk.
  sourceFiles.forEach(f => fs.accessSync(f));
  // Since we also want to rebuild the bundle if rollup configuration or the the
  // tooling changes (e.g. when typescript is updated), add the currently loaded
  // node.js modules to the list of dependencies.
  let inputs = [...sourceFiles, ...configFiles, ...Object.keys(require.cache)];
  // Deduplicate the list of inputs.
  inputs = Array.from(new Set(inputs.map(f => path.resolve(f))));
  // Turn filenames into relative paths and format/escape them for a Makefile.
  inputs = inputs.map(formatPath);

  // Build a list of output filenames and normalize those too.
  const depFile = path.join(
    outputDir,
    path.basename(outputFile, path.extname(outputFile)) + ".d"
  );
  const outputs = [outputFile, depFile].map(formatPath);

  // Generate depfile contents.
  const depFileContent = [
    ...outputs.map(filename => `${filename}: ` + inputs.join(" ") + "\n\n"),
    ...inputs.map(filename => `${filename}:\n`)
  ].join("");

  // Since we're writing the depfile when node's "beforeExit" hook triggers,
  // it's getting written _after_ the regular outputs are saved to disk.
  // Therefore, after writing the depfile, reset its timestamps to when we last
  // discovered a dependency, which was certainly before the bundle was built.
  fs.writeFileSync(depFile, depFileContent);
  fs.utimesSync(depFile, timestamp, timestamp);

  // Renders path to make it suitable for a depfile.
  function formatPath(filename) {
    // Make the path relative to the root build directory.
    filename = path.relative(buildDir, filename);
    // Use forward slashes on Windows.
    if (process.platform === "win32") {
      filename = filename.replace(/\\/g, "/");
    }
    // Escape spaces with a backslash. This is what rust and clang do too.
    filename = filename.replace(/ /g, "\\ ");
    return filename;
  }
}

export default function makeConfig(commandOptions) {
  return {
    output: {
      format: "iife",
      name: "denoMain",
      sourcemap: true,
      sourcemapExcludeSources: true
    },

    plugins: [
      // inject platform and arch from Node
      platform({
        include: [platformPath]
      }),

      // would prefer to use `rollup-plugin-virtual` to inject the empty module, but there
      // is an issue with `rollup-plugin-commonjs` which causes errors when using the
      // virtual plugin (see: rollup/rollup-plugin-commonjs#315), this means we have to use
      // a physical module to substitute
      alias({
        fs: mockPath,
        path: mockPath,
        os: mockPath,
        crypto: mockPath,
        buffer: mockPath,
        module: mockPath
      }),

      // Provides inlining of file contents for `js/assets.ts`
      strings({
        include: [
          "*.d.ts",
          `${__dirname}/**/*.d.ts`,
          `${process.cwd()}/**/*.d.ts`
        ]
      }),

      // Resolves any resources that have been generated at build time
      resolveGenerated(),

      // Allows rollup to resolve modules based on Node.js resolution
      nodeResolve({
        jsnext: true,
        main: true
      }),

      // Allows rollup to import CommonJS modules
      commonjs({
        namedExports: {
          // Static analysis of `typescript.js` does detect the exports properly, therefore
          // rollup requires them to be explicitly defined to make them available in the
          // bundle
          [typescriptPath]: [
            "createLanguageService",
            "formatDiagnosticsWithColorAndContext",
            "ModuleKind",
            "ScriptKind",
            "ScriptSnapshot",
            "ScriptTarget",
            "version"
          ]
        }
      }),

      typescriptPlugin({
        // The build script is invoked from `out/:target` so passing an absolute file path is needed
        tsconfig,

        // This provides any overrides to the `tsconfig.json` that are needed to bundle
        tsconfigOverride,

        // This provides the locally configured version of TypeScript instead of the plugins
        // default version
        typescript,

        // By default, the include path only includes the cwd and below, need to include the root of the project
        // and build path to be passed to this plugin.  This is different front tsconfig.json include
        include: ["*.ts", `${__dirname}/**/*.ts`, `${process.cwd()}/**/*.ts`],

        // d.ts files are not bundled and by default like include, it only includes the cwd and below
        exclude: [
          "*.d.ts",
          `${__dirname}/**/*.d.ts`,
          `${process.cwd()}/**/*.d.ts`
        ]
      }),

      // Provide some concise information about the bundle
      analyze({
        skipFormatted: true,
        onAnalysis({
          bundleSize,
          bundleOrigSize,
          bundleReduction,
          moduleCount
        }) {
          if (!commandOptions.silent) {
            console.log(
              `Bundle size: ${Math.round((bundleSize / 1000000) * 100) / 100}Mb`
            );
            console.log(
              `Original size: ${Math.round((bundleOrigSize / 1000000) * 100) /
                100}Mb`
            );
            console.log(`Reduction: ${bundleReduction}%`);
            console.log(`Module count: ${moduleCount}`);
          }
        }
      }),

      // source-map-support, which is required by TypeScript to support source maps, requires Node.js Buffer
      // implementation.  This needs to come at the end of the plugins because of the impact it has on
      // the existing runtime environment, which breaks other plugins and features of the bundler.
      globals(),

      generateDepFile({
        outputFile: commandOptions.o,
        configFiles: [commandOptions.c, tsconfig]
      })
    ]
  };
}
