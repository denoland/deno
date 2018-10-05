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

const mockPath = path.join(__dirname, "js", "mock_builtin.js");
const platformPath = path.join(__dirname, "js", "platform.ts");
const tsconfig = path.join(__dirname, "tsconfig.json");
const typescriptPath = `${
  process.env.BASEPATH
}/third_party/node_modules/typescript/lib/typescript.js`;

// We will allow generated modules to be resolvable by TypeScript based on
// the current build path
const tsconfigOverride = {
  compilerOptions: {
    paths: {
      "*": ["*", path.join(process.cwd(), "*")]
    }
  }
};

// this is a preamble for the `globals.d.ts` file to allow it to be the default
// lib for deno.
const libPreamble = `/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
`;

// this is a rollup plugin which will look for imports ending with `!string` and resolve
// them with a module that will inline the contents of the file as a string.  Needed to
// support `js/assets.ts`.
function strings({ include, exclude } = {}) {
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
          return path.resolve(path.join(process.env.BASEPATH, importee));
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
          code: `export default ${JSON.stringify(
            id.endsWith("globals.d.ts") ? libPreamble + code : code
          )};`,
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

// Inject deno.platform.arch and deno.platform.os
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
        const resolved = path.resolve(
          path.join(process.cwd(), `${importee}.ts`)
        );
        return resolved;
      }
    }
  };
}

export default function makeConfig(commandOptions) {
  return {
    output: {
      format: "iife",
      name: "denoMain",
      sourcemap: true
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
      globals()
    ]
  };
}
