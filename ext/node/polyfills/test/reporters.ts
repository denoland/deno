// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
"use strict";

const { core, primordials } = __bootstrap;
const {
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ObjectAssign,
  ReflectConstruct,
  SafeArrayIterator,
  SafeRegExp,
  String,
  StringPrototypeReplace,
  StringPrototypeSplit,
  SymbolAsyncIterator,
} = primordials;

const yamlEscapePattern = new SafeRegExp("\\\\", "g");
const xmlEscapePattern = new SafeRegExp("[<>&\"']", "g");
const {
  tapEscape,
  tapIndent,
} = core.loadExtScript("ext:deno_node/internal/test/reporters.ts");

let _Transform = null;
function getTransform() {
  if (_Transform === null) {
    _Transform = core.loadExtScript(
      "ext:deno_node/internal/streams/transform.js",
    ).Transform;
  }
  return _Transform;
}

function yamlEscape(value) {
  return StringPrototypeReplace(String(value), yamlEscapePattern, "\\\\");
}

function xmlEscape(value) {
  return StringPrototypeReplace(String(value), xmlEscapePattern, (char) => {
    switch (char) {
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case "&":
        return "&amp;";
      case '"':
        return "&quot;";
      case "'":
        return "&apos;";
    }
    return char;
  });
}

function formatError(details) {
  if (details === undefined || details === null) return "";
  // Only a `details.error` is rendered as a TAP error block. A bare details
  // object (carrying duration_ms/type for a passing test) is not an error.
  const error = details.error;
  if (error === undefined || error === null) return "";
  if (typeof error === "string") return error;
  return error.stack ?? error.message ?? String(error);
}

function reportTapDetails(nesting, data) {
  const details = formatError(data.details);
  const durationMs = data.details === undefined || data.details === null
    ? undefined
    : data.details.duration_ms;
  const hasDuration = typeof durationMs === "number";
  if (
    !details && data.skip === undefined && data.todo === undefined &&
    !hasDuration
  ) {
    return "";
  }
  const lines = [];
  const prefix = tapIndent(nesting);
  ArrayPrototypePush(lines, `${prefix}  ---\n`);
  if (hasDuration) {
    ArrayPrototypePush(lines, `${prefix}  duration_ms: ${durationMs}\n`);
  }
  if (data.skip !== undefined) {
    ArrayPrototypePush(lines, `${prefix}  skip: ${yamlEscape(data.skip)}\n`);
  }
  if (data.todo !== undefined) {
    ArrayPrototypePush(lines, `${prefix}  todo: ${yamlEscape(data.todo)}\n`);
  }
  if (details) {
    ArrayPrototypePush(lines, `${prefix}  error: |-\n`);
    for (
      const line of new SafeArrayIterator(
        StringPrototypeSplit(String(details), "\n"),
      )
    ) {
      ArrayPrototypePush(lines, `${prefix}    ${yamlEscape(line)}\n`);
    }
  }
  ArrayPrototypePush(lines, `${prefix}  ...\n`);
  return ArrayPrototypeJoin(lines, "");
}

function reportTest(status, data) {
  const nesting = data.nesting || 0;
  const directive = data.skip !== undefined
    ? ` # SKIP${data.skip ? ` ${tapEscape(data.skip)}` : ""}`
    : data.todo !== undefined
    ? ` # TODO${data.todo ? ` ${tapEscape(data.todo)}` : ""}`
    : "";
  const testNumber = data.testNumber ? ` ${data.testNumber}` : "";
  return `${tapIndent(nesting)}${status}${testNumber} - ${
    tapEscape(data.name ?? "<anonymous>")
  }${directive}\n`;
}

async function* tap(source) {
  yield "TAP version 13\n";
  const tapIterator = source[SymbolAsyncIterator]();
  while (true) {
    // deno-lint-ignore prefer-primordials
    const { done, value: event } = await tapIterator.next();
    if (done) break;
    const { type, data } = event;
    switch (type) {
      case "test:plan":
        yield `${tapIndent(data.nesting)}1..${data.count}\n`;
        break;
      case "test:start":
        yield `${tapIndent(data.nesting)}# Subtest: ${tapEscape(data.name)}\n`;
        break;
      case "test:pass":
        yield reportTest("ok", data);
        {
          const details = reportTapDetails(data.nesting || 0, data);
          if (details) yield details;
        }
        break;
      case "test:fail":
        yield reportTest("not ok", data);
        {
          const details = reportTapDetails(data.nesting || 0, data);
          if (details) yield details;
        }
        break;
      case "test:diagnostic":
        yield `${tapIndent(data.nesting)}# ${tapEscape(data.message)}\n`;
        break;
      case "test:stdout":
      case "test:stderr":
        yield String(data.message ?? data);
        break;
    }
  }
}

async function* dot(source) {
  let sawTests = false;
  const dotIterator = source[SymbolAsyncIterator]();
  while (true) {
    // deno-lint-ignore prefer-primordials
    const { done, value } = await dotIterator.next();
    if (done) break;
    const { type } = value;
    if (type === "test:pass") {
      sawTests = true;
      yield ".";
    } else if (type === "test:fail") {
      sawTests = true;
      yield "X";
    }
  }
  if (sawTests) yield "\n";
}

async function* junit(source) {
  const testcases = [];
  const junitIterator = source[SymbolAsyncIterator]();
  while (true) {
    // deno-lint-ignore prefer-primordials
    const { done, value } = await junitIterator.next();
    if (done) break;
    const { type, data } = value;
    if (type !== "test:pass" && type !== "test:fail") continue;
    const name = xmlEscape(data.name ?? "<anonymous>");
    const duration = data.duration_ms === undefined
      ? ""
      : ` time="${xmlEscape(data.duration_ms / 1000)}"`;
    if (type === "test:pass") {
      ArrayPrototypePush(
        testcases,
        `  <testcase name="${name}"${duration}/>\n`,
      );
    } else {
      const message = xmlEscape(formatError(data.details));
      ArrayPrototypePush(
        testcases,
        `  <testcase name="${name}"${duration}><failure>${message}</failure></testcase>\n`,
      );
    }
  }
  yield '<?xml version="1.0" encoding="utf-8"?>\n';
  yield "<testsuites>\n";
  yield ` <testsuite tests="${testcases.length}">\n`;
  for (const testcase of new SafeArrayIterator(testcases)) yield testcase;
  yield " </testsuite>\n";
  yield "</testsuites>\n";
}

class SpecReporter extends getTransform() {
  constructor(options = { __proto__: null }) {
    super(ObjectAssign({ writableObjectMode: true }, options));
  }

  _transform(event, _encoding, callback) {
    try {
      const { type, data } = event;
      if (type === "test:pass") {
        // deno-lint-ignore prefer-primordials
        this.push(`${tapIndent(data.nesting)}ok ${data.name}\n`);
      } else if (type === "test:fail") {
        // deno-lint-ignore prefer-primordials
        this.push(`${tapIndent(data.nesting)}not ok ${data.name}\n`);
      } else if (type === "test:diagnostic") {
        // deno-lint-ignore prefer-primordials
        this.push(`${tapIndent(data.nesting)}# ${data.message}\n`);
      }
      callback();
    } catch (err) {
      callback(err);
    }
  }
}

class LcovReporter extends getTransform() {
  constructor(options = { __proto__: null }) {
    super(ObjectAssign({ writableObjectMode: true }, options));
  }

  _transform(_event, _encoding, callback) {
    callback();
  }
}

function makeConstructorWrapper(ctor) {
  return function reporterConstructorWrapper() {
    if (new.target) {
      return ReflectConstruct(ctor, arguments, new.target);
    }
    return ReflectConstruct(ctor, arguments);
  };
}

const spec = makeConstructorWrapper(SpecReporter);
spec.prototype = SpecReporter.prototype;
const lcov = makeConstructorWrapper(LcovReporter);
lcov.prototype = LcovReporter.prototype;

const defaultExport = {
  __proto__: null,
  dot,
  junit,
  lcov,
  spec,
  tap,
};

return {
  dot,
  junit,
  lcov,
  spec,
  tap,
  default: defaultExport,
};
})();
