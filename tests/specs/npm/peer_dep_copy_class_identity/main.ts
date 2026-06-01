// Regression test for denoland/deno#26427. Pulling the same package
// (peer-dep-test-class) in via two peer-dependency contexts used to create
// two copy folders under node_modules/.deno with their own physical copies
// of the package source. Each copy produced a distinct class object, so
// libraries like NestJS (which use decorator metadata to wire up DI) saw
// `SharedToken !== SharedToken` and could not resolve providers.
//
// After the fix in libs/npm_installer/local.rs, the copy variants are
// symlinks to a single canonical package directory, so realpath()
// deduplicates them and the class identity is stable.

import classConsumer1 from "npm:@denotest/peer-dep-test-class-consumer@1";
import classConsumer2 from "npm:@denotest/peer-dep-test-class-consumer@2";
import * as directClass from "npm:@denotest/peer-dep-test-class@1";

const { SharedToken: viaConsumer1 } = classConsumer1;
const { SharedToken: viaConsumer2 } = classConsumer2;
const { SharedToken: viaDirect } = directClass;

console.log("consumer1 === consumer2:", viaConsumer1 === viaConsumer2);
console.log("consumer1 === direct:", viaConsumer1 === viaDirect);
console.log("consumer2 === direct:", viaConsumer2 === viaDirect);
