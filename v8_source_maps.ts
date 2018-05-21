import { SourceMapConsumer, MappedPosition } from "source-map";
import * as base64 from "base64-js";

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

    const position = mapSourcePosition({
      source,
      line,
      column
    });
    frame = cloneCallSite(frame);
    frame.getFileName = () => position.source;
    frame.getLineNumber = () => position.line;
    frame.getColumnNumber = () => Number(position.column) + 1;
    frame.getScriptNameOrSourceURL = () => position.source;
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
    obj[name] = /^(?:is|get|toString)/.test(name)
      ? () => frame_[name].call(frame)
      : frame_[name];
  });
  return (obj as any) as CallSite;
  // tslint:enable:no-any
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
      //sourceMapData = bufferFrom(rawData, "base64").toString();
      const ui8 = base64.toByteArray(rawData);
      sourceMapData = arrayToStr(ui8);
      sourceMappingURL = source;
    } else {
      // Support source map URLs relative to the source URL
      //sourceMappingURL = supportRelativeURL(source, sourceMappingURL);
      //sourceMapData = retrieveFile(sourceMappingURL);
    }

    const rawSourceMap = JSON.parse(sourceMapData);
    consumer = new SourceMapConsumer(rawSourceMap);
    consumers.set(source, consumer);
  }
  return consumer;
}

const consumers = new Map<string, SourceMapConsumer>();

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

// TODO move to util?
function arrayToStr(ui8: Uint8Array): string {
  return String.fromCharCode.apply(null, ui8);
}
