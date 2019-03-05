// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./vendor/standalone.js";
import "./vendor/parser_typescript.js";
import "./vendor/parser_babylon.js";
import "./vendor/parser_markdown.js";

// TODO: provide decent type declarions for these
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { prettier, prettierPlugins } = window as any;

export { prettier, prettierPlugins };
