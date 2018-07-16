import path from "path";
import alias from "rollup-plugin-alias";
import commonjs from "rollup-plugin-commonjs";
import nodeResolve from "rollup-plugin-node-resolve";
import typescript from "rollup-plugin-typescript2";

const mockPath = path.join(path.resolve("../../js/"), "mock_builtin");

export default {
  output: {
    format: "iife",
    name: "denoMain",
    sourcemap: true
  },

  plugins: [
    alias({
      fs: mockPath,
      path: mockPath,
      os: mockPath,
      crypto: mockPath,
      buffer: mockPath,
      module: mockPath
    }),

    nodeResolve({
      jsnext: true,
      main: true
    }),

    commonjs({
      namedExports: {
        "../../third_party/node_modules/typescript/lib/typescript.js": [ "version" ]
      }
    }),

    typescript({
      // Move the cache to the OS"s temporary directory
      cacheRoot: `${require("os").tmpdir()}/.rpt2_cache`,

      // The build script is invoked from `out/Target` and so config is located from the CWD
      tsconfig: "../../tsconfig.json",

      // By default, the include path only includes the cwd and below, need to include the root of the project
      // to be passed to this plugin.  This is different front tsconfig.json include
      include: [ "*.ts+(|x)", "../../**/*.ts+(|x)" ],

      // d.ts files are not bundled and by default like include, it only includes the cwd and below
      exclude: [ "*.d.ts", "../../**/*.d.ts" ]
    })
  ]
}
