import { assetSourceCode } from "../../js/assets";

// tslint:disable:max-line-length
import libDts from "/third_party/node_modules/typescript/lib/lib.d.ts!string";
import libDomDts from "/third_party/node_modules/typescript/lib/lib.dom.d.ts!string";
import libDomIterableDts from "/third_party/node_modules/typescript/lib/lib.dom.iterable.d.ts!string";
import libEs6Dts from "/third_party/node_modules/typescript/lib/lib.es6.d.ts!string";
import libEs2016FullDts from "/third_party/node_modules/typescript/lib/lib.es2016.full.d.ts!string";
import libEs2017FullDts from "/third_party/node_modules/typescript/lib/lib.es2017.full.d.ts!string";
import libEs2018FullDts from "/third_party/node_modules/typescript/lib/lib.es2018.full.d.ts!string";
import libEsNextFullDts from "/third_party/node_modules/typescript/lib/lib.esnext.full.d.ts!string";
import libScripthostDts from "/third_party/node_modules/typescript/lib/lib.scripthost.d.ts!string";
import libWebworkerDts from "/third_party/node_modules/typescript/lib/lib.webworker.d.ts!string";
import libWebworkerImportscriptsDts from "/third_party/node_modules/typescript/lib/lib.webworker.importscripts.d.ts!string";
// tslint:enable

assetSourceCode["lib.d.ts"] = libDts;
assetSourceCode["lib.dom.d.ts"] = libDomDts;
assetSourceCode["lib.dom.iterable.d.ts"] = libDomIterableDts;
assetSourceCode["lib.es6.d.ts"] = libEs6Dts;
assetSourceCode["lib.es2016.full.d.ts"] = libEs2016FullDts;
assetSourceCode["lib.es2017.full.d.ts"] = libEs2017FullDts;
assetSourceCode["lib.es2018.full.d.ts"] = libEs2018FullDts;
assetSourceCode["lib.esnext.full.d.ts"] = libEsNextFullDts;
assetSourceCode["lib.scripthost.d.ts"] = libScripthostDts;
assetSourceCode["lib.webworker.d.ts"] = libWebworkerDts;
assetSourceCode[
  "lib.webworker.importscripts.d.ts"
] = libWebworkerImportscriptsDts;

export function readAsset(path: string): string {
  path = path.replace(ASSET, "");
  if (path in assetSourceCode) {
    return assetSourceCode[path];
  } else {
    throw new RangeError(`The asset "${path}" not in asset bundle.`);
  }
}
export const ASSET = "/$asset$/";
