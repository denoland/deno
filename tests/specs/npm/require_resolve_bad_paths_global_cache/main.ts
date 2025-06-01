import "npm:@denotest/esm-basic";
import { resolve } from "npm:@denotest/require-resolve";

console.log(resolve("@denotest/esm-basic", {
  // when using the global cache, it should fallback to resolving bare
  // specifiers with the global cache when it can't find it via the paths
  paths: ["/home"],
}));
