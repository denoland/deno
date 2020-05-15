const filenameBase = "test_plugin";

let filenameSuffix = ".so";
let filenamePrefix = "lib";

if (Deno.build.os === "windows") {
  filenameSuffix = ".dll";
  filenamePrefix = "";
}
if (Deno.build.os === "darwin") {
  filenameSuffix = ".dylib";
}

const filename = `../target/${Deno.args[0]}/${filenamePrefix}${filenameBase}${filenameSuffix}`;

// This will be checked against open resources after Plugin.close()
// in runTestClose() below.
const resourcesPre = Deno.resources();

const pluginRid = Deno.openPlugin(filename);

const { testSync, testAsync, jsonTest } = Deno.core.ops();
if (!(testSync > 0)) {
  throw "bad op id for testSync";
}
if (!(testAsync > 0)) {
  throw "bad op id for testAsync";
}
if (!(jsonTest > 0)) {
  throw "bad op id for jsonTest";
}

export { testSync, testAsync, jsonTest };

/**
 * Close/drop plugin resource and check that it isn't kept loaded.
 * This should be run after every plugin test to ensure that closing
 * the plugin is compatible with all features.
 */
export function runTestPluginClose() {
  Deno.close(pluginRid);

  const resourcesPost = Deno.resources();

  const preStr = JSON.stringify(resourcesPre, null, 2);
  const postStr = JSON.stringify(resourcesPost, null, 2);
  if (preStr !== postStr) {
    throw new Error(`Difference in open resources before openPlugin and after Plugin.close(): 
Before: ${preStr}
After: ${postStr}`);
  }
}
