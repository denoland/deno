import path from "path";
import alias from "rollup-plugin-alias";
import { plugin as analyze } from "rollup-plugin-analyzer";
import commonjs from "rollup-plugin-commonjs";
import nodeResolve from "rollup-plugin-node-resolve";
import typescript from "rollup-plugin-typescript2";

const mockPath = path.join(__dirname, "js", "mock_builtin");
const tsconfig = path.join(__dirname, "tsconfig.json");
const typescriptPath = `${process.env.BASEPATH}/node_modules/typescript/lib/typescript.js`;

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
        [typescriptPath]: [ "version" ]
      }
    }),

    typescript({
      // The build script is invoked from `out/Target` and so config is located alongside this file
      tsconfig,

      // By default, the include path only includes the cwd and below, need to include the root of the project
      // to be passed to this plugin.  This is different front tsconfig.json include
      include: [ "*.ts", `${__dirname}/**/*.ts` ],

      // d.ts files are not bundled and by default like include, it only includes the cwd and below
      exclude: [ "*.d.ts", `${__dirname}/**/*.d.ts` ]
    }),

    analyze({
      skipFormatted: true,
      onAnalysis({bundleSize, bundleOrigSize, bundleReduction, moduleCount}) {
        console.log(`Bundle size: ${Math.round(bundleSize/1000000*100)/100}Mb`);
        console.log(`Original size: ${Math.round(bundleOrigSize/1000000*100)/100}Mb`);
        console.log(`Reduction: ${bundleReduction}%`);
        console.log(`Module count: ${moduleCount}`);
      }
    })
  ]
}
