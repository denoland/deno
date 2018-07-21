// this is a rollup plugin which will look for imports ending with `!string` and resolve
// them with a module that will inline the contents of the file as a string.  Needed to
// support `js/assets.ts` and used by `rollup.config.js`

import path from "path";
import { createFilter } from "rollup-pluginutils";

export default function strings({ include, exclude } = {}) {
  if (!include) {
    throw new Error("include option must be passed");
  }

  const filter = createFilter(include, exclude);

  return {
    name: "strings",

    resolveId(importee) {
      if (importee.endsWith("!string")) {
        return path.resolve(
          path.join(
            process.env.BASEPATH,
            importee.slice(0, importee.lastIndexOf("!string"))
          )
        );
      }
    },

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
