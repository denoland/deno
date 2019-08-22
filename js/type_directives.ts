// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

interface DirectiveInfo {
  path: string;
  start: number;
  end: number;
}

/** Remap the module name based on any supplied type directives passed. */
export function getMappedModuleName(
  moduleName: string,
  containingFile: string,
  typeDirectives?: Record<string, string>
): string {
  if (containingFile.endsWith(".d.ts") && !moduleName.endsWith(".d.ts")) {
    moduleName = `${moduleName}.d.ts`;
  }
  if (!typeDirectives) {
    return moduleName;
  }
  if (moduleName in typeDirectives) {
    return typeDirectives[moduleName];
  }
  return moduleName;
}

/** Matches directives that look something like this and parses out the value
 * of the directive:
 *
 *      // @deno-types="./foo.d.ts"
 *
 * [See Diagram](http://bit.ly/31nZPCF)
 */
const typeDirectiveRegEx = /@deno-types\s*=\s*(["'])((?:(?=(\\?))\3.)*?)\1/gi;

/** Matches `import` or `export from` statements and parses out the value of the
 * module specifier in the second capture group:
 *
 *      import * as foo from "./foo.js"
 *      export { a, b, c } from "./bar.js"
 *
 * [See Diagram](http://bit.ly/2GSkJlF)
 */
const importExportRegEx = /(?:import|export)\s+[\s\S]*?from\s+(["'])((?:(?=(\\?))\3.)*?)\1/;

/** Parses out any Deno type directives that are part of the source code, or
 * returns `undefined` if there are not any.
 */
export function parseTypeDirectives(
  sourceCode: string | undefined
): Record<string, string> | undefined {
  if (!sourceCode) {
    return;
  }

  // collect all the directives in the file and their start and end positions
  const directives: DirectiveInfo[] = [];
  let maybeMatch: RegExpExecArray | null = null;
  while ((maybeMatch = typeDirectiveRegEx.exec(sourceCode))) {
    const [matchString, , path] = maybeMatch;
    const { index: start } = maybeMatch;
    directives.push({
      path,
      start,
      end: start + matchString.length
    });
  }
  if (!directives.length) {
    return;
  }

  // work from the last directive backwards for the next `import`/`export`
  // statement
  directives.reverse();
  const directiveRecords: Record<string, string> = {};
  for (const { path, start, end } of directives) {
    const searchString = sourceCode.substring(end);
    const maybeMatch = importExportRegEx.exec(searchString);
    if (maybeMatch) {
      const [, , fromPath] = maybeMatch;
      directiveRecords[fromPath] = path;
    }
    sourceCode = sourceCode.substring(0, start);
  }

  return directiveRecords;
}
