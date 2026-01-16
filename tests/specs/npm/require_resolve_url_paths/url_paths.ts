import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

console.log(getParentUrl());
console.log(require.resolve("@denotest/esm-basic", {
  paths: [getParentUrl()],
}));

function getParentUrl() {
  const fileUrl = import.meta.url;
  return fileUrl.substring(0, fileUrl.lastIndexOf("/") + 1);
}
