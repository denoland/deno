// Type definitions for prettier 1.18
// Project: https://github.com/prettier/prettier, https://prettier.io
// Definitions by: Ika <https://github.com/ikatyang>,
//                 Ifiok Jr. <https://github.com/ifiokjr>,
//                 Florian Keller <https://github.com/ffflorian>
// Definitions: https://github.com/DefinitelyTyped/DefinitelyTyped
// TypeScript Version: 2.8

export type AST = any;
export type Doc = doc.builders.Doc;

// https://github.com/prettier/prettier/blob/master/src/common/fast-path.js
export interface FastPath<T = any> {
    stack: any[];
    getName(): null | PropertyKey;
    getValue(): T;
    getNode(count?: number): null | T;
    getParentNode(count?: number): null | T;
    call<U>(callback: (path: this) => U, ...names: PropertyKey[]): U;
    each(callback: (path: this) => void, ...names: PropertyKey[]): void;
    map<U>(callback: (path: this, index: number) => U, ...names: PropertyKey[]): U[];
}

export type BuiltInParser = (text: string, options?: any) => AST;
export type BuiltInParserName =
    | 'babylon' // deprecated
    | 'babel'
    | 'babel-flow'
    | 'flow'
    | 'typescript'
    | 'postcss' // deprecated
    | 'css'
    | 'less'
    | 'scss'
    | 'json'
    | 'json5'
    | 'json-stringify'
    | 'graphql'
    | 'markdown'
    | 'vue'
    | 'html'
    | 'angular'
    | 'mdx'
    | 'yaml'
    | 'lwc';

export type CustomParser = (text: string, parsers: Record<BuiltInParserName, BuiltInParser>, options: Options) => AST;

export interface Options extends Partial<RequiredOptions> {}
export interface RequiredOptions extends doc.printer.Options {
    /**
     * Print semicolons at the ends of statements.
     */
    semi: boolean;
    /**
     * Use single quotes instead of double quotes.
     */
    singleQuote: boolean;
    /**
     * Use single quotes in JSX.
     */
    jsxSingleQuote: boolean;
    /**
     * Print trailing commas wherever possible.
     */
    trailingComma: 'none' | 'es5' | 'all';
    /**
     * Print spaces between brackets in object literals.
     */
    bracketSpacing: boolean;
    /**
     * Put the `>` of a multi-line JSX element at the end of the last line instead of being alone on the next line.
     */
    jsxBracketSameLine: boolean;
    /**
     * Format only a segment of a file.
     */
    rangeStart: number;
    /**
     * Format only a segment of a file.
     */
    rangeEnd: number;
    /**
     * Specify which parser to use.
     */
    parser: BuiltInParserName | CustomParser;
    /**
     * Specify the input filepath. This will be used to do parser inference.
     */
    filepath: string;
    /**
     * Prettier can restrict itself to only format files that contain a special comment, called a pragma, at the top of the file.
     * This is very useful when gradually transitioning large, unformatted codebases to prettier.
     */
    requirePragma: boolean;
    /**
     * Prettier can insert a special @format marker at the top of files specifying that
     * the file has been formatted with prettier. This works well when used in tandem with
     * the --require-pragma option. If there is already a docblock at the top of
     * the file then this option will add a newline to it with the @format marker.
     */
    insertPragma: boolean;
    /**
     * By default, Prettier will wrap markdown text as-is since some services use a linebreak-sensitive renderer.
     * In some cases you may want to rely on editor/viewer soft wrapping instead, so this option allows you to opt out.
     */
    proseWrap:
        | boolean // deprecated
        | 'always'
        | 'never'
        | 'preserve';
    /**
     * Include parentheses around a sole arrow function parameter.
     */
    arrowParens: 'avoid' | 'always';
    /**
     * The plugin API is in a beta state.
     */
    plugins: Array<string | Plugin>;
    /**
     * How to handle whitespaces in HTML.
     */
    htmlWhitespaceSensitivity: 'css' | 'strict' | 'ignore';
    /**
     * Which end of line characters to apply.
     */
    endOfLine: 'auto' | 'lf' | 'crlf' | 'cr';
    /**
     * Change when properties in objects are quoted.
     */
    quoteProps: 'as-needed' | 'consistent' | 'preserve';
}

export interface ParserOptions extends RequiredOptions {
    locStart: (node: any) => number;
    locEnd: (node: any) => number;
    originalText: string;
}

export interface Plugin {
    languages?: SupportLanguage[];
    parsers?: { [parserName: string]: Parser };
    printers?: { [astFormat: string]: Printer };
    options?: SupportOption[];
    defaultOptions?: Partial<RequiredOptions>;
}

export interface Parser {
    parse: (text: string, parsers: { [parserName: string]: Parser }, options: ParserOptions) => AST;
    astFormat: string;
    hasPragma?: (text: string) => boolean;
    locStart: (node: any) => number;
    locEnd: (node: any) => number;
    preprocess?: (text: string, options: ParserOptions) => string;
}

export interface Printer {
    print(
        path: FastPath,
        options: ParserOptions,
        print: (path: FastPath) => Doc,
    ): Doc;
    embed?: (
        path: FastPath,
        print: (path: FastPath) => Doc,
        textToDoc: (text: string, options: Options) => Doc,
        options: ParserOptions,
    ) => Doc | null;
    insertPragma?: (text: string) => string;
    /**
     * @returns `null` if you want to remove this node
     * @returns `void` if you want to use modified newNode
     * @returns anything if you want to replace the node with it
     */
    massageAstNode?: (node: any, newNode: any, parent: any) => any;
    hasPrettierIgnore?: (path: FastPath) => boolean;
    canAttachComment?: (node: any) => boolean;
    willPrintOwnComments?: (path: FastPath) => boolean;
    printComments?: (path: FastPath, print: (path: FastPath) => Doc, options: ParserOptions, needsSemi: boolean) => Doc;
    handleComments?: {
        ownLine?: (commentNode: any, text: string, options: ParserOptions, ast: any, isLastComment: boolean) => boolean;
        endOfLine?: (commentNode: any, text: string, options: ParserOptions, ast: any, isLastComment: boolean) => boolean;
        remaining?: (commentNode: any, text: string, options: ParserOptions, ast: any, isLastComment: boolean) => boolean;
    };
}

export interface CursorOptions extends Options {
    /**
     * Specify where the cursor is.
     */
    cursorOffset: number;
    rangeStart?: never;
    rangeEnd?: never;
}

export interface CursorResult {
    formatted: string;
    cursorOffset: number;
}

/**
 * `format` is used to format text using Prettier. [Options](https://github.com/prettier/prettier#options) may be provided to override the defaults.
 */
export function format(source: string, options?: Options): string;

/**
 * `check` checks to see if the file has been formatted with Prettier given those options and returns a `Boolean`.
 * This is similar to the `--list-different` parameter in the CLI and is useful for running Prettier in CI scenarios.
 */
export function check(source: string, options?: Options): boolean;

/**
 * `formatWithCursor` both formats the code, and translates a cursor position from unformatted code to formatted code.
 * This is useful for editor integrations, to prevent the cursor from moving when code is formatted.
 *
 * The `cursorOffset` option should be provided, to specify where the cursor is. This option cannot be used with `rangeStart` and `rangeEnd`.
 */
export function formatWithCursor(source: string, options: CursorOptions): CursorResult;

export interface ResolveConfigOptions {
    /**
     * If set to `false`, all caching will be bypassed.
     */
    useCache?: boolean;
    /**
     * Pass directly the path of the config file if you don't wish to search for it.
     */
    config?: string;
    /**
     * If set to `true` and an `.editorconfig` file is in your project,
     * Prettier will parse it and convert its properties to the corresponding prettier configuration.
     * This configuration will be overridden by `.prettierrc`, etc. Currently,
     * the following EditorConfig properties are supported:
     * - indent_style
     * - indent_size/tab_width
     * - max_line_length
     */
    editorconfig?: boolean;
}

/**
 * `resolveConfig` can be used to resolve configuration for a given source file,
 * passing its path as the first argument. The config search will start at the
 * file path and continue to search up the directory.
 * (You can use `process.cwd()` to start searching from the current directory).
 *
 * A promise is returned which will resolve to:
 *
 *  - An options object, providing a [config file](https://github.com/prettier/prettier#configuration-file) was found.
 *  - `null`, if no file was found.
 *
 * The promise will be rejected if there was an error parsing the configuration file.
 */
export function resolveConfig(filePath: string, options?: ResolveConfigOptions): Promise<null | Options>;
export namespace resolveConfig {
    function sync(filePath: string, options?: ResolveConfigOptions): null | Options;
}

/**
 * As you repeatedly call `resolveConfig`, the file system structure will be cached for performance. This function will clear the cache.
 * Generally this is only needed for editor integrations that know that the file system has changed since the last format took place.
 */
export function clearConfigCache(): void;

export interface SupportLanguage {
    name: string;
    since?: string;
    parsers: BuiltInParserName[] | string[];
    group?: string;
    tmScope?: string;
    aceMode?: string;
    codemirrorMode?: string;
    codemirrorMimeType?: string;
    aliases?: string[];
    extensions?: string[];
    filenames?: string[];
    linguistLanguageId?: number;
    vscodeLanguageIds?: string[];
}

export interface SupportOptionDefault {
    since: string;
    value: SupportOptionValue;
}

export interface SupportOption {
    since?: string;
    type: 'int' | 'boolean' | 'choice' | 'path';
    array?: boolean;
    deprecated?: string;
    redirect?: SupportOptionRedirect;
    description: string;
    oppositeDescription?: string;
    default: SupportOptionValue | SupportOptionDefault[];
    range?: SupportOptionRange;
    choices?: SupportOptionChoice[];
    category: string;
}

export interface SupportOptionRedirect {
    options: string;
    value: SupportOptionValue;
}

export interface SupportOptionRange {
    start: number;
    end: number;
    step: number;
}

export interface SupportOptionChoice {
    value: boolean | string;
    description?: string;
    since?: string;
    deprecated?: string;
    redirect?: SupportOptionValue;
}

export type SupportOptionValue = number | boolean | string;

export interface SupportInfo {
    languages: SupportLanguage[];
    options: SupportOption[];
}

export interface FileInfoOptions {
    ignorePath?: string;
    withNodeModules?: boolean;
    plugins?: string[];
}

export interface FileInfoResult {
    ignored: boolean;
    inferredParser: string | null;
}

export function getFileInfo(filePath: string, options?: FileInfoOptions): Promise<FileInfoResult>;

export namespace getFileInfo {
    function sync(filePath: string, options?: FileInfoOptions): FileInfoResult;
}

/**
 * Returns an object representing the parsers, languages and file types Prettier supports.
 * If `version` is provided (e.g. `"1.5.0"`), information for that version will be returned,
 * otherwise information for the current version will be returned.
 */
export function getSupportInfo(version?: string): SupportInfo;

/**
 * `version` field in `package.json`
 */
export const version: string;

// https://github.com/prettier/prettier/blob/master/src/common/util-shared.js
export namespace util {
    function isNextLineEmpty(text: string, node: any, options: ParserOptions): boolean;
    function isNextLineEmptyAfterIndex(text: string, index: number): boolean;
    function getNextNonSpaceNonCommentCharacterIndex(text: string, node: any, options: ParserOptions): number;
    function makeString(rawContent: string, enclosingQuote: "'" | '"', unescapeUnnecessaryEscapes: boolean): string;
    function addLeadingComment(node: any, commentNode: any): void;
    function addDanglingComment(node: any, commentNode: any): void;
    function addTrailingComment(node: any, commentNode: any): void;
}

// https://github.com/prettier/prettier/blob/master/src/doc/index.js
export namespace doc {
    namespace builders {
        type Doc =
            | string
            | Align
            | BreakParent
            | Concat
            | Fill
            | Group
            | IfBreak
            | Indent
            | Line
            | LineSuffix
            | LineSuffixBoundary;

        interface Align {
            type: 'align';
            contents: Doc;
            n: number | string | { type: 'root' };
        }

        interface BreakParent {
            type: 'break-parent';
        }

        interface Concat {
            type: 'concat';
            parts: Doc[];
        }

        interface Fill {
            type: 'fill';
            parts: Doc[];
        }

        interface Group {
            type: 'group';
            contents: Doc;
            break: boolean;
            expandedStates: Doc[];
        }

        interface IfBreak {
            type: 'if-break';
            breakContents: Doc;
            flatContents: Doc;
        }

        interface Indent {
            type: 'indent';
            contents: Doc;
        }

        interface Line {
            type: 'line';
            soft?: boolean;
            hard?: boolean;
            literal?: boolean;
        }

        interface LineSuffix {
            type: 'line-suffix';
            contents: Doc;
        }

        interface LineSuffixBoundary {
            type: 'line-suffix-boundary';
        }

        function addAlignmentToDoc(doc: Doc, size: number, tabWidth: number): Doc;
        function align(n: Align['n'], contents: Doc): Align;
        const breakParent: BreakParent;
        function concat(contents: Doc[]): Concat;
        function conditionalGroup(states: Doc[], opts?: { shouldBreak: boolean }): Group;
        function dedent(contents: Doc): Align;
        function dedentToRoot(contents: Doc): Align;
        function fill(parts: Doc[]): Fill;
        function group(contents: Doc, opts?: { shouldBreak: boolean }): Group;
        const hardline: Concat;
        function ifBreak(breakContents: Doc, flatContents: Doc): IfBreak;
        function indent(contents: Doc): Indent;
        function join(separator: Doc, parts: Doc[]): Concat;
        const line: Line;
        function lineSuffix(contents: Doc): LineSuffix;
        const lineSuffixBoundary: LineSuffixBoundary;
        const literalline: Concat;
        function markAsRoot(contents: Doc): Align;
        const softline: Line;
    }
    namespace debug {
        function printDocToDebug(doc: Doc): string;
    }
    namespace printer {
        function printDocToString(doc: Doc, options: Options): {
            formatted: string;
            cursorNodeStart?: number;
            cursorNodeText?: string;
        };
        interface Options {
            /**
             * Specify the line length that the printer will wrap on.
             */
            printWidth: number;
            /**
             * Specify the number of spaces per indentation-level.
             */
            tabWidth: number;
            /**
             * Indent lines with tabs instead of spaces
             */
            useTabs: boolean;
        }
    }
    namespace utils {
        function isEmpty(doc: Doc): boolean;
        function isLineNext(doc: Doc): boolean;
        function willBreak(doc: Doc): boolean;
        function traverseDoc(doc: Doc, onEnter?: (doc: Doc) => void | boolean, onExit?: (doc: Doc) => void, shouldTraverseConditionalGroups?: boolean): void;
        function mapDoc<T>(doc: Doc, callback: (doc: Doc) => T): T;
        function propagateBreaks(doc: Doc): void;
        function removeLines(doc: Doc): Doc;
        function stripTrailingHardline(doc: Doc): Doc;
    }
}
