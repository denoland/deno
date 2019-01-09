// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// This allows TypeScript to resolve any modules that end with `!string`
// as there is a rollup plugin that will take any mids ending with `!string`
// and return them as a string to rollup for inlining
declare module "*!string" {
  const value: string;
  export default value;
}
