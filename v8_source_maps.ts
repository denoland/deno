// Copyright 2014 Evan Wallace
// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
// Originated from source-map-support but has been heavily modified for deno.
import { SourceMapConsumer, MappedPosition } from "source-map";
import * as base64 from "base64-js";
import { arrayToStr } from "./util";

const consumers = new Map<string, SourceMapConsumer>();

interface Options {
  // A callback the returns generated file contents.
  getGeneratedContents: GetGeneratedContentsCallback;
  // Usually set the following to true. Set to false for testing.
  installPrepareStackTrace: boolean;
}

interface CallSite extends NodeJS.CallSite {
  getScriptNameOrSourceURL(): string;
}

interface Position {
  source: string; // Filename
  column: number;
  line: number;
}

type GetGeneratedContentsCallback = (fileName: string) => string;

let getGeneratedContents: GetGeneratedContentsCallback;

export function install(options: Options) {
  getGeneratedContents = options.getGeneratedContents;
  if (options.installPrepareStackTrace) {
    Error.prepareStackTrace = prepareStackTraceWrapper;
  }
}

export function prepareStackTraceWrapper(
  error: Error,
  stack: CallSite[]
): string {
  try {
    return prepareStackTrace(error, stack);
  } catch (prepareStackError) {
    Error.prepareStackTrace = null;
    console.log("=====Error inside of prepareStackTrace====");
    console.log(prepareStackError.stack.toString());
    console.log("=====Original error=======================");
    throw error;
  }
}

export function prepareStackTrace(error: Error, stack: CallSite[]): string {
  const frames = stack.map(
    (frame: CallSite) => "\n    at " + wrapCallSite(frame).toString()
  );
  return error.toString() + frames.join("");
}

export function wrapCallSite(frame: CallSite): CallSite {
  if (frame.isNative()) {
    return frame;
  }

  // Most call sites will return the source file from getFileName(), but code
  // passed to eval() ending in "//# sourceURL=..." will return the source file
  // from getScriptNameOrSourceURL() instead
  const source = frame.getFileName() || frame.getScriptNameOrSourceURL();

  if (source) {
    const line = frame.getLineNumber();
    const column = frame.getColumnNumber() - 1;
    const position = mapSourcePosition({ source, line, column });
    frame = cloneCallSite(frame);
    frame.getFileName = () => position.source;
    frame.getLineNumber = () => position.line;
    frame.getColumnNumber = () => Number(position.column) + 1;
    frame.getScriptNameOrSourceURL = () => position.source;
    frame.toString = () => CallSiteToString(frame);
    return frame;
  }

  // Code called using eval() needs special handling
  let origin = frame.isEval() && frame.getEvalOrigin();
  if (origin) {
    origin = mapEvalOrigin(origin);
    frame = cloneCallSite(frame);
    frame.getEvalOrigin = () => origin;
    return frame;
  }

  // If we get here then we were unable to change the source position
  return frame;
}

function cloneCallSite(frame: CallSite): CallSite {
  // tslint:disable:no-any
  const obj: any = {};
  const frame_ = frame as any;
  const props = Object.getOwnPropertyNames(Object.getPrototypeOf(frame));
  props.forEach(name => {
    obj[name] = /^(?:is|get)/.test(name)
      ? () => frame_[name].call(frame)
      : frame_[name];
  });
  return (obj as any) as CallSite;
  // tslint:enable:no-any
}

// Taken from source-map-support, original copied from V8's messages.js
// MIT License. Copyright (c) 2014 Evan Wallace
function CallSiteToString(frame: CallSite): string {
  let fileName;
  let fileLocation = "";
  if (frame.isNative()) {
    fileLocation = "native";
  } else {
    fileName = frame.getScriptNameOrSourceURL();
    if (!fileName && frame.isEval()) {
      fileLocation = frame.getEvalOrigin();
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
      fileLocation += ":" + String(lineNumber);
      const columnNumber = frame.getColumnNumber();
      if (columnNumber) {
        fileLocation += ":" + String(columnNumber);
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
    // Fixes shim to be backward compatable with Node v0 to v4
    if (typeName === "[object Object]") {
      typeName = "null";
    }
    const methodName = frame.getMethodName();
    if (functionName) {
      if (typeName && functionName.indexOf(typeName) !== 0) {
        line += typeName + ".";
      }
      line += functionName;
      if (
        methodName &&
        functionName.indexOf("." + methodName) !==
          functionName.length - methodName.length - 1
      ) {
        line += " [as " + methodName + "]";
      }
    } else {
      line += typeName + "." + (methodName || "<anonymous>");
    }
  } else if (isConstructor) {
    line += "new " + (functionName || "<anonymous>");
  } else if (functionName) {
    line += functionName;
  } else {
    line += fileLocation;
    addSuffix = false;
  }
  if (addSuffix) {
    line += " (" + fileLocation + ")";
  }
  return line;
}

// Regex for detecting source maps
const reSourceMap = /^data:application\/json[^,]+base64,/;

function loadConsumer(source: string): SourceMapConsumer {
  let consumer = consumers.get(source);
  if (consumer == null) {
    const code = getGeneratedContents(source);
    if (!code) {
      return null;
    }

    let sourceMappingURL = retrieveSourceMapURL(code);
    if (!sourceMappingURL) {
      throw Error("No source map?");
    }

    let sourceMapData: string;
    if (reSourceMap.test(sourceMappingURL)) {
      // Support source map URL as a data url
      const rawData = sourceMappingURL.slice(sourceMappingURL.indexOf(",") + 1);
      const ui8 = base64.toByteArray(rawData);
      sourceMapData = arrayToStr(ui8);
      sourceMappingURL = source;
    } else {
      // Support source map URLs relative to the source URL
      //sourceMappingURL = supportRelativeURL(source, sourceMappingURL);
      sourceMapData = getGeneratedContents(sourceMappingURL);
    }

    //console.log("sourceMapData", sourceMapData);
    const rawSourceMap = JSON.parse(sourceMapData);
    consumer = new SourceMapConsumer(rawSourceMap);
    consumers.set(source, consumer);
  }
  return consumer;
}

function retrieveSourceMapURL(fileData: string): string {
  // Get the URL of the source map
  // tslint:disable-next-line:max-line-length
  const re = /(?:\/\/[@#][ \t]+sourceMappingURL=([^\s'"]+?)[ \t]*$)|(?:\/\*[@#][ \t]+sourceMappingURL=([^\*]+?)[ \t]*(?:\*\/)[ \t]*$)/gm;
  // Keep executing the search to find the *last* sourceMappingURL to avoid
  // picking up sourceMappingURLs from comments, strings, etc.
  let lastMatch, match;
  while ((match = re.exec(fileData))) {
    lastMatch = match;
  }
  if (!lastMatch) {
    return null;
  }
  return lastMatch[1];
}

function mapSourcePosition(position: Position): MappedPosition {
  const consumer = loadConsumer(position.source);
  if (consumer == null) {
    return position;
  }
  const mapped = consumer.originalPositionFor(position);
  return mapped;
}

// Parses code generated by FormatEvalOrigin(), a function inside V8:
// https://code.google.com/p/v8/source/browse/trunk/src/messages.js
function mapEvalOrigin(origin: string): string {
  // Most eval() calls are in this format
  let match = /^eval at ([^(]+) \((.+):(\d+):(\d+)\)$/.exec(origin);
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
  match = /^eval at ([^(]+) \((.+)\)$/.exec(origin);
  if (match) {
    return "eval at " + match[1] + " (" + mapEvalOrigin(match[2]) + ")";
  }

  // Make sure we still return useful information if we didn't find anything
  return origin;
}
