// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Node's implementation checks for a symbol they put in the blob prototype
// Since the implementation of Blob is Deno's, the only option is to check the
// objects constructor
export function isBlob(object) {
  return object instanceof Blob;
}
