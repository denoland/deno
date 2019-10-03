// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

interface FileReference {
  fileName: string;
  pos: number;
  end: number;
}

/** Remap the module name based on any supplied type directives passed. */
export function getMappedModuleName(
  source: FileReference,
  typeDirectives: Map<FileReference, string>
): string {
  const { fileName: sourceFileName, pos: sourcePos } = source;
  for (const [{ fileName, pos }, value] of typeDirectives.entries()) {
    if (sourceFileName === fileName && sourcePos === pos) {
      return value;
    }
  }
  return source.fileName;
}

/** Matches directives that look something like this and parses out the value
 * of the directive:
 *
 *      // @deno-types="./foo.d.ts"
 *
 * [See Diagram](http://bit.ly/31nZPCF)
 */
const typeDirectiveRegEx = /@deno-types\s*=\s*(["'])((?:(?=(\\?))\3.)*?)\1/gi;

/** Matches `import`, `import from` or `export from` statements and parses out the value of the
 * module specifier in the second capture group:
 *
 *      import "./foo.js"
 *      import * as foo from "./foo.js"
 *      export { a, b, c } from "./bar.js"
 *
 * [See Diagram](http://bit.ly/2lOsp0K)
 */
const importExportRegEx = /(?:import|export)(?:\s+|\s+[\s\S]*?from\s+)?(["'])((?:(?=(\\?))\3.)*?)\1/;

/** Parses out any Deno type directives that are part of the source code, or
 * returns `undefined` if there are not any.
 */
export function parseTypeDirectives(
  sourceCode: string | undefined
): Map<FileReference, string> | undefined {
  if (!sourceCode) {
    return;
  }

  // collect all the directives in the file and their start and end positions
  const directives: FileReference[] = [];
  let maybeMatch: RegExpExecArray | null = null;
  while ((maybeMatch = typeDirectiveRegEx.exec(sourceCode))) {
    const [matchString, , fileName] = maybeMatch;
    const { index: pos } = maybeMatch;
    directives.push({
      fileName,
      pos,
      end: pos + matchString.length
    });
  }
  if (!directives.length) {
    return;
  }

  // work from the last directive backwards for the next `import`/`export`
  // statement
  directives.reverse();
  const results = new Map<FileReference, string>();
  for (const { end, fileName, pos } of directives) {
    const searchString = sourceCode.substring(end);
    const maybeMatch = importExportRegEx.exec(searchString);
    if (maybeMatch) {
      const [matchString, , targetFileName] = maybeMatch;
      const targetPos =
        end + maybeMatch.index + matchString.indexOf(targetFileName) - 1;
      const target: FileReference = {
        fileName: targetFileName,
        pos: targetPos,
        end: targetPos + targetFileName.length
      };
      results.set(target, fileName);
    }
    sourceCode = sourceCode.substring(0, pos);
  }

  return results;
}
