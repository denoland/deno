import { core } from "ext:core/mod.js";
// import { build } from "./bundle/mod.ts";
// TODO(mmastrac): We cannot import these from "ext:core/ops" yet
const {
  op_bundle_resolve,
  op_bundle_load,
} = core.ops;

console.log("bundle", core.ops);

globalThis.Deno.bundle = {
  resolve: op_bundle_resolve,
  load: op_bundle_load,
};

// export function bundle(entryPoints, outfile) {
//   return build({
//     entryPoints,
//     outfile,
//     bundle: true,
//     format: "esm",
//     packages: "bundle",
//     treeShaking: true,
//     plugins: [
//       {
//         name: "test",
//         setup(build) {

//         }
//       }
//     ]
//   });
// }
// 

async function bundle(entryPoints, outfile) {
  await build({
    // entryPoints: ["/Users/nathanwhit/Documents/Code/dev-tools/main.ts"],
    entryPoints: ["./testing.ts"],
    outfile: "./temp/mod.js",
    bundle: true,
    format: "esm",
    packages: "bundle",
    minifyIdentifiers: false,
    minifySyntax: false,
    minifyWhitespace: true,
    treeShaking: true,
    plugins: [
      {
        name: "test",
        setup(build) {
          build.onResolve({ filter: /.*$/ }, (args) => {
            console.log("test plugin resolve", args);
            return null;
          });
          build.onLoad({ filter: /.*$/ }, (args) => {
            console.log("test plugin load", args);
            return null;
          });
        },
      },
    ],
  })
}

// exposes the functions that are called when the compiler is used as a
// language service.
globalThis.bundle = bundle;
