// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./standalone.js";
import "./parser_typescript.js";
import "./parser_markdown.js";

// TODO: provide decent type declarions for these
const { prettier, prettierPlugins } = window as any;

export { prettier, prettierPlugins };
