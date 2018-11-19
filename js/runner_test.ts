import { test, assert, assertEqual } from "./test_util";
import * as deno from "deno";

// tslint:disable-next-line:no-any
const Runner = (deno as any)._runner.Runner;

let mockGetOutputStack: string[] = [];
let mockGetFilenameStack: Array<[string, string]> = [];

// tslint:disable:max-line-length
const mockOutput = {
  "/root/project/foo/bar.ts": `define(["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    exports.foo = "bar";
});
//# sourceMappingURL=bar.js.map
//# sourceURL=/root/project/foo/bar.ts`,
  "/root/project/foo/baz.ts": `define(["require", "exports", "./qat.ts"], function (require, exports, qat) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    exports.foo = qat.foo;
});
//# sourceMappingURL=baz.js.map
//# sourceURL=/root/project/foo/baz.ts`,
  "/root/project/foo/qat.ts": `define(["require", "exports"], function (require, exports) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.foo = "qat";
});
//# sourceMappingURL=qat.js.map
//# sourceURL=/root/project/foo/qat.ts`,
  "/root/project/foo/config.json": `define([], function () {
  return JSON.parse('{"foo":{"bar": true,"baz": ["qat", 1]}}');
});
//# sourceURL=/root/project/foo/config.json`,
  "/circular/modA.ts": `define(["require", "exports", "./modB.ts"], function (require, exports, modB) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.foo = modB.foo;
});
//# sourceMappingURL=modA.js.map
//# sourceURL=/circular/modA.ts`,
  "/circular/modB.ts": `define(["require", "exports", "./modA.ts"], function (require, exports, modA) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  if (modA && typeof modA === "object") {
    modA;
  } else {
    throw new Error("Mod A is empty!");
  }
  exports.foo = "bar";
});
//# sourceMappingURL=modB.js.map
//# sourceURL=/circular/modB.ts`
};
// tslint:enable

const mockFilenames = {
  "/root/project": {
    "foo/bar.ts": "/root/project/foo/bar.ts",
    "foo/baz.ts": "/root/project/foo/baz.ts",
    "foo/config.json": "/root/project/foo/config.json"
  },
  "/root/project/foo/baz.ts": {
    "./qat.ts": "/root/project/foo/qat.ts"
  },
  "/circular": {
    "modA.ts": "/circular/modA.ts"
  },
  "/circular/modA.ts": {
    "./modB.ts": "/circular/modB.ts"
  },
  "/circular/modB.ts": {
    "./modA.ts": "/circular/modA.ts"
  }
};

const mockCodeProvider = {
  getOutput(filename: string) {
    mockGetOutputStack.push(filename);
    if (filename in mockOutput) {
      return mockOutput[filename];
    }
    throw new Error("Module not found.");
  },
  getFilename(moduleSpecifier: string, containingFile: string) {
    mockGetFilenameStack.push([moduleSpecifier, containingFile]);
    if (
      containingFile in mockFilenames &&
      moduleSpecifier in mockFilenames[containingFile]
    ) {
      return mockFilenames[containingFile][moduleSpecifier];
    }
  }
};

function setup() {
  mockGetOutputStack = [];
  mockGetFilenameStack = [];
  return new Runner(mockCodeProvider);
}

test(function runnerConstruction() {
  const runner = setup();
  assert(runner);
});

test(function runnerRun() {
  const runner = setup();
  const result = runner.run("foo/bar.ts", "/root/project");
  assertEqual(result, { foo: "bar" });
  assertEqual(mockGetFilenameStack, [["foo/bar.ts", "/root/project"]]);
  assertEqual(mockGetOutputStack, ["/root/project/foo/bar.ts"]);
});

test(function runnerRunImports() {
  const runner = setup();
  const result = runner.run("foo/baz.ts", "/root/project");
  assertEqual(result, { foo: "qat" });
  assertEqual(mockGetFilenameStack, [
    ["foo/baz.ts", "/root/project"],
    ["./qat.ts", "/root/project/foo/baz.ts"]
  ]);
  assertEqual(mockGetOutputStack, [
    "/root/project/foo/baz.ts",
    "/root/project/foo/qat.ts"
  ]);
});

test(function runnerRunExportReturn() {
  const runner = setup();
  const result = runner.run("foo/config.json", "/root/project");
  assertEqual(result, { foo: { bar: true, baz: ["qat", 1] } });
});

test(function runnerCircularReference() {
  const runner = setup();
  const result = runner.run("modA.ts", "/circular");
  assertEqual(result, { foo: "bar" });
});
