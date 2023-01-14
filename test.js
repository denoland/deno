import { Parcel } from "npm:@parcel/core";
let bundler = new Parcel({
  entries: "test.js",
  defaultConfig: "@parcel/config-default",
});
console.log(bundler);
let { bundleGraph, buildTime } = await bundler.run();
let bundles = bundleGraph.getBundles();
console.log(`✨ Built ${bundles.length} bundles in ${buildTime}ms!`);
