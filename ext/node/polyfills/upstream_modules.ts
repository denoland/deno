// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Upstream modules
const callerPath = `const callerCallsite = require("caller-callsite");
const re = /^file:/;

module.exports = () => {
  const fileUrl = callerCallsite().getFileName();
  return fileUrl.replace(re, "");
};
`;

// From: https://github.com/stefanpenner/get-caller-file/blob/2383bf9e98ed3c568ff69d7586cf59c0f1dcb9d3/index.ts
const getCallerFile = `
const re = /^file:\\/\\//;

module.exports = function getCallerFile(position = 2) {
  if (position >= Error.stackTraceLimit) {
    throw new TypeError('getCallerFile(position) requires position be less then Error.stackTraceLimit but position was: "' + position + '" and Error.stackTraceLimit was: "' + Error.stackTraceLimit + '"');
  }

  const oldPrepareStackTrace = Error.prepareStackTrace;
  Error.prepareStackTrace = (_, stack)  => stack;
  const stack = new Error().stack;
  Error.prepareStackTrace = oldPrepareStackTrace;


  if (stack !== null && typeof stack === 'object') {
    // stack[0] holds this file
    // stack[1] holds where this function was called
    // stack[2] holds the file we're interested in
    return stack[position] ? stack[position].getFileName().replace(re, "") : undefined;
  }
};
`;

export default {
  "caller-path": callerPath,
  "get-caller-file": getCallerFile,
} as Record<string, string>;
