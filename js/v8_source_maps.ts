// Copyright 2014 Evan Wallace
// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Originated from source-map-support but has been heavily modified for deno.

import { SourceMapConsumer, MappedPosition } from "source-map";
import { CallSite, RawSourceMap } from "./types";
import { atob } from "./text_encoding";

const consumers = new Map<string, SourceMapConsumer>();

interface Options {
  // A callback the returns generated file contents.
  getGeneratedContents: GetGeneratedContentsCallback;
  // Usually set the following to true. Set to false for testing.
  installPrepareStackTrace: boolean;
}

interface Position {
  source: string; // Filename
  column: number;
  line: number;
}

type GetGeneratedContentsCallback = (fileName: string) => string | RawSourceMap;

let getGeneratedContents: GetGeneratedContentsCallback;

// @internal
export function install(options: Options) {
  getGeneratedContents = options.getGeneratedContents;
  if (options.installPrepareStackTrace) {
    Error.prepareStackTrace = prepareStackTraceWrapper;
  }
}

// @internal
export function prepareStackTraceWrapper(
  error: Error,
  stack: CallSite[]
): string {
  try {
    return prepareStackTrace(error, stack);
  } catch (prepareStackError) {
    Error.prepareStackTrace = undefined;
    console.log("=====Error inside of prepareStackTrace====");
    console.log(prepareStackError.stack.toString());
    console.log("=====Original error=======================");
    throw error;
  }
}

// @internal
export function prepareStackTrace(error: Error, stack: CallSite[]): string {
  const frames = stack.map(
    frame => `\n    at ${wrapCallSite(frame).toString()}`
  );
  return `${error.toString()}${frames.join("")}`;
}

// @internal
export function wrapCallSite(frame: CallSite): CallSite {
  if (frame.isNative()) {
    return frame;
  }

  // Most call sites will return the source file from getFileName(), but code
  // passed to eval() ending in "//# sourceURL=..." will return the source file
  // from getScriptNameOrSourceURL() instead
  const source = frame.getFileName() || frame.getScriptNameOrSourceURL();

  if (source) {
    const line = frame.getLineNumber() || 0;
    const column = (frame.getColumnNumber() || 1) - 1;
    const position = mapSourcePosition({ source, line, column });
    frame = cloneCallSite(frame);
    Object.assign(frame, {
      getFileName: () => position.source,
      getLineNumber: () => position.line,
      getColumnNumber: () => Number(position.column) + 1,
      getScriptNameOrSourceURL: () => position.source,
      toString: () => CallSiteToString(frame)
    });
    return frame;
  }

  // Code called using eval() needs special handling
  let origin = (frame.isEval() && frame.getEvalOrigin()) || undefined;
  if (origin) {
    origin = mapEvalOrigin(origin);
    frame = cloneCallSite(frame);
    Object.assign(frame, {
      getEvalOrigin: () => origin,
      toString: () => CallSiteToString(frame)
    });
    return frame;
  }

  // If we get here then we were unable to change the source position
  return frame;
}

function cloneCallSite(
  frame: CallSite
  // mixin: Partial<CallSite> & { toString: () => string }
): CallSite {
  const obj = {} as CallSite;
  const props = Object.getOwnPropertyNames(
    Object.getPrototypeOf(frame)
  ) as Array<keyof CallSite>;
  for (const name of props) {
    obj[name] = /^(?:is|get)/.test(name)
      ? () => frame[name].call(frame)
      : frame[name];
  }
  return obj;
}

// Taken from source-map-support, original copied from V8's messages.js
// MIT License. Copyright (c) 2014 Evan Wallace
function CallSiteToString(frame: CallSite): string {
  let fileLocation = "";
  if (frame.isNative()) {
    fileLocation = "native";
  } else {
    const fileName = frame.getScriptNameOrSourceURL();
    if (!fileName && frame.isEval()) {
      fileLocation = frame.getEvalOrigin() || "";
      fileLocation += ", "; // Expecting source position to follow.
    }

    if (fileName) {
      fileLocation += fileName;
    } else {
      // Source code does not originate from a file and is not native, but we
      // can still get the source position inside the source string, e.g. in
      // an eval string.
      fileLocation += "<anonymous>";
    }
    const lineNumber = frame.getLineNumber();
    if (lineNumber != null) {
      fileLocation += `:${lineNumber}`;
      const columnNumber = frame.getColumnNumber();
      if (columnNumber) {
        fileLocation += `:${columnNumber}`;
      }
    }
  }

  let line = "";
  const functionName = frame.getFunctionName();
  let addSuffix = true;
  const isConstructor = frame.isConstructor();
  const isMethodCall = !(frame.isToplevel() || isConstructor);
  if (isMethodCall) {
    let typeName = frame.getTypeName();
    // Fixes shim to be backward compatible with Node v0 to v4
    if (typeName === "[object Object]") {
      typeName = "null";
    }
    const methodName = frame.getMethodName();
    if (functionName) {
      if (typeName && functionName.indexOf(typeName) !== 0) {
        line += `${typeName}.`;
      }
      line += functionName;
      if (
        methodName &&
        functionName.indexOf("." + methodName) !==
          functionName.length - methodName.length - 1
      ) {
        line += ` [as ${methodName} ]`;
      }
    } else {
      line += `${typeName}.${methodName || "<anonymous>"}`;
    }
  } else if (isConstructor) {
    line += `new ${functionName || "<anonymous>"}`;
  } else if (functionName) {
    line += functionName;
  } else {
    line += fileLocation;
    addSuffix = false;
  }
  if (addSuffix) {
    line += ` (${fileLocation})`;
  }
  return line;
}

// Regex for detecting source maps
const reSourceMap = /^data:application\/json[^,]+base64,/;

export function loadConsumer(source: string): SourceMapConsumer | null {
  let consumer = consumers.get(source);
  if (consumer == null) {
    const code = getGeneratedContents(source);
    if (!code) {
      return null;
    }
    if (typeof code !== "string") {
      throw new Error("expected string");
    }

    let sourceMappingURL = retrieveSourceMapURL(code);
    if (!sourceMappingURL) {
      throw Error("No source map?");
    }

    let sourceMapData: string | RawSourceMap;
    if (reSourceMap.test(sourceMappingURL)) {
      // Support source map URL as a data url
      const rawData = sourceMappingURL.slice(sourceMappingURL.indexOf(",") + 1);
      sourceMapData = atob(rawData);
      sourceMappingURL = source;
    } else {
      // TODO Support source map URLs relative to the source URL
      // sourceMappingURL = supportRelativeURL(source, sourceMappingURL);
      sourceMapData = getGeneratedContents(sourceMappingURL);
    }

    const rawSourceMap =
      typeof sourceMapData === "string"
        ? (JSON.parse(sourceMapData) as RawSourceMap)
        : sourceMapData;
    consumer = new SourceMapConsumer(rawSourceMap);
    consumers.set(source, consumer);
  }
  return consumer;
}

// tslint:disable-next-line:max-line-length
const sourceMapUrlRe = /(?:\/\/[@#][ \t]+sourceMappingURL=([^\s'"]+?)[ \t]*$)|(?:\/\*[@#][ \t]+sourceMappingURL=([^\*]+?)[ \t]*(?:\*\/)[ \t]*$)/gm;

function retrieveSourceMapURL(fileData: string): string | null {
  // Keep executing the search to find the *last* sourceMappingURL to avoid
  // picking up sourceMappingURLs from comments, strings, etc.
  let lastMatch, match;
  while ((match = sourceMapUrlRe.exec(fileData))) {
    lastMatch = match;
  }
  if (!lastMatch) {
    return null;
  }
  return lastMatch[1];
}

export function mapSourcePosition(position: Position): MappedPosition {
  const consumer = loadConsumer(position.source);
  if (consumer == null) {
    return position;
  }
  return consumer.originalPositionFor(position);
}

const stackEvalRe = /^eval at ([^(]+) \((.+):(\d+):(\d+)\)$/;
const nestedEvalRe = /^eval at ([^(]+) \((.+)\)$/;

// Parses code generated by FormatEvalOrigin(), a function inside V8:
// https://code.google.com/p/v8/source/browse/trunk/src/messages.js
function mapEvalOrigin(origin: string): string {
  // Most eval() calls are in this format
  let match = stackEvalRe.exec(origin);
  if (match) {
    const position = mapSourcePosition({
      source: match[2],
      line: Number(match[3]),
      column: Number(match[4]) - 1
    });
    const pos = [
      position.source,
      position.line,
      Number(position.column) + 1
    ].join(":");
    return `eval at ${match[1]} (${pos})`;
  }

  // Parse nested eval() calls using recursion
  match = nestedEvalRe.exec(origin);
  if (match) {
    return `eval at ${match[1]} (${mapEvalOrigin(match[2])})`;
  }

  // Make sure we still return useful information if we didn't find anything
  return origin;
}
