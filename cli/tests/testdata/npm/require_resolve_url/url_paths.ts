import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

console.log(getParentUrl());
console.log(resolveWithPath(getParentUrl()));

function resolveWithPath(rootUrl) {
  return require.resolve("@denotest/esm-basic", {
    paths: [rootUrl],
  });
}

function getParentUrl() {
  const fileUrl = import.meta.url;
  return fileUrl.substring(0, fileUrl.lastIndexOf("/") + 1);
}
