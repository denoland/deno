// Ported from marked - a markdown parser
// Copyright (c) 2011-2018, Christopher Jeffrey. (MIT Licensed)
// https://github.com/markedjs/marked

import { assert } from "../testing/asserts.ts";

export type Callback = (e: Error | null, out?: string) => void;

/**
 * An optional function which can provide intelligent highlighting to blocks
 * of code. */
export interface Highlighter {
  /**
   * @param str The string to be highlighted
   * @param lang A string representing the language
   * @param callback A callback to provide the highlighted string
   */
  (str: string, lang?: string, callback?: Callback): string | undefined;
}

/** Options which can be set within the library. */
export interface MarkdownOptions {
  /** A prefix url for any relative link.  Defaults to `null`. */
  baseUrl: string | null;

  /** If true, add `<br>` on a single line break (copies GitHub). Requires
   * `gfm` be `true`.  Defaults to `false`. */
  breaks: boolean;

  /** If true, use approved
   * [GitHub Flavored Markdown (GFM) specification](https://github.github.com/gfm/).
   * Defaults to `true`. */
  gfm: boolean;

  /** If `true`, include an `id` attribute when emitting headings (`h1`, `h2`,
   * `h3`, etc). */
  headerIds: boolean;

  /** A string to prefix the `id` attribute when emitting headings (`h1`, `h2`,
   * `h3`, etc). Defaults to `""`. */
  headerPrefix: string;

  /** A function to highlight code blocks. Defaults to `null`. */
  highlight: Highlighter | null;

  /** A string to prefix the `className` in a `<code>` block. Useful for syntax
   * highlighting.  Defaults to `"language-"`. */
  langPrefix: string;

  /** If `true`, autolinked email address is escaped with HTML character
   * references. Defaults to `true`. */
  mangle: boolean;

  /** If true, conform to the original `markdown.pl` as much as possible. Don't
   * fix original markdown bugs or behavior. Turns off and overrides `gfm`.
   * Defaults to `false`. */
  pedantic: boolean;

  /** An object containing functions to render tokens to HTML. */
  renderer: Renderer;

  /** If `true`, the parser does not throw any exception. Defaults to `false`.*/
  silent: boolean;

  /** If `true`, use smarter list behavior than those found in `markdown.pl`.
   * Defaults to `false`. */
  smartLists: boolean;

  /** If `true`, use "smart" typographic punctuation for things like quotes and
   * dashes. Defaults to `false`. */
  smartypants: boolean;

  /** If true, emit self-closing HTML tags for void elements (`<br/>`, `<img/>`,
   * etc.) with a "/" as required by XHTML. Defaults to `false`. */
  xhtml: boolean;
}

interface TableCellFlags {
  header: boolean;
  align: "left" | "right" | "center" | null;
}

interface ListItemStart {
  type: "list_item_start";
  task: boolean;
  checked?: boolean;
  loose: boolean;
}

type Token =
  | {
      type:
        | "space"
        | "hr"
        | "blockquote_start"
        | "blockquote_end"
        | "list_item_end"
        | "list_end";
    }
  | {
      type: "paragraph" | "html" | "text";
      text: string;
    }
  | {
      type: "code";
      codeBlockStyle?: "indented";
      lang?: string;
      text: string;
      escaped?: boolean;
    }
  | {
      type: "heading";
      depth: number;
      text: string;
    }
  | {
      type: "table";
      header: string[];
      align: Array<"left" | "right" | "center" | null>;
      cells: string[][];
    }
  | {
      type: "list_start";
      ordered: boolean;
      start?: number;
      loose: boolean;
    }
  | ListItemStart;

interface Link {
  href: string;
  title: string;
}

type Tokens = Token[] & {
  links: Record<string, Link>;
};

interface EditChain {
  replace(name: RegExp | string, val: RegExp | string): EditChain;
  getRegex(): RegExp;
}

function createTokens(): Tokens {
  const tokens = ([] as unknown) as Tokens;
  tokens.links = Object.create(null);
  return tokens;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const noop = function noop() {} as any;
noop.exec = noop;

const block: Record<string, RegExp> = {
  newline: /^\n+/,
  code: /^( {4}[^\n]+\n*)+/,
  fences: /^ {0,3}(`{3,}|~{3,})([^`~\n]*)\n(?:|([\s\S]*?)\n)(?: {0,3}\1[~`]* *(?:\n+|$)|$)/,
  hr: /^ {0,3}((?:- *){3,}|(?:_ *){3,}|(?:\* *){3,})(?:\n+|$)/,
  heading: /^ {0,3}(#{1,6}) +([^\n]*?)(?: +#+)? *(?:\n+|$)/,
  blockquote: /^( {0,3}> ?(paragraph|[^\n]*)(?:\n|$))+/,
  list: /^( {0,3})(bull) [\s\S]+?(?:hr|def|\n{2,}(?! )(?!\1bull )\n*|\s*$)/,
  def: /^ {0,3}\[(label)\]: *\n? *<?([^\s>]+)>?(?:(?: +\n? *| *\n *)(title))? *(?:\n+|$)/,
  nptable: noop,
  table: noop,
  lheading: /^([^\n]+)\n {0,3}(=+|-+) *(?:\n+|$)/,
  // regex template, placeholders will be replaced according to different paragraph
  // interruption rules of commonmark and the original markdown spec:
  _paragraph: /^([^\n]+(?:\n(?!hr|heading|lheading|blockquote|fences|list|html)[^\n]+)*)/,
  _label: /(?!\s*\])(?:\\[\[\]]|[^\[\]])+/,
  _title: /(?:"(?:\\"?|[^"\\])*"|'[^'\n]*(?:\n[^'\n]+)*\n?'|\([^()]*\))/,
  text: /^[^\n]+/
};

block.def = edit(block.def)
  .replace("label", block._label)
  .replace("title", block._title)
  .getRegex();

block.bullet = /(?:[*+-]|\d{1,9}\.)/;
block.item = /^( *)(bull) ?[^\n]*(?:\n(?!\1bull ?)[^\n]*)*/;
block.item = edit(block.item, "gm")
  .replace(/bull/g, block.bullet)
  .getRegex();

block.list = edit(block.list)
  .replace(/bull/g, block.bullet)
  .replace(
    "hr",
    "\\n+(?=\\1?(?:(?:- *){3,}|(?:_ *){3,}|(?:\\* *){3,})(?:\\n+|$))"
  )
  .replace("def", "\\n+(?=" + block.def.source + ")")
  .getRegex();

const blockHtml =
  "^ {0,3}(?:" + // optional indentation
  "<(script|pre|style)[\\s>][\\s\\S]*?(?:</\\1>[^\\n]*\\n+|$)" + // (1)
  "|comment[^\\n]*(\\n+|$)" + // (2)
  "|<\\?[\\s\\S]*?\\?>\\n*" + // (3)
  "|<![A-Z][\\s\\S]*?>\\n*" + // (4)
  "|<!\\[CDATA\\[[\\s\\S]*?\\]\\]>\\n*" + // (5)
  "|</?(tag)(?: +|\\n|/?>)[\\s\\S]*?(?:\\n{2,}|$)" + // (6)
  "|<(?!script|pre|style)([a-z][\\w-]*)(?:attribute)*? */?>(?=[ \\t]*(?:\\n|$))[\\s\\S]*?(?:\\n{2,}|$)" + // (7) open tag
  "|</(?!script|pre|style)[a-z][\\w-]*\\s*>(?=[ \\t]*(?:\\n|$))[\\s\\S]*?(?:\\n{2,}|$)" + // (7) closing tag
  ")";
const blockTag =
  "address|article|aside|base|basefont|blockquote|body|caption" +
  "|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption" +
  "|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe" +
  "|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option" +
  "|p|param|section|source|summary|table|tbody|td|tfoot|th|thead|title|tr" +
  "|track|ul";
block._comment = /<!--(?!-?>)[\s\S]*?-->/;
block.html = edit(blockHtml, "i")
  .replace("comment", block._comment)
  .replace("tag", blockTag)
  .replace(
    "attribute",
    / +[a-zA-Z:_][\w.:-]*(?: *= *"[^"\n]*"| *= *'[^'\n]*'| *= *[^\s"'=<>`]+)?/
  )
  .getRegex();

block.paragraph = edit(block._paragraph)
  .replace("hr", block.hr)
  .replace("heading", " {0,3}#{1,6} +")
  .replace("|lheading", "") // setex headings don't interrupt commonmark paragraphs
  .replace("blockquote", " {0,3}>")
  .replace("fences", " {0,3}(?:`{3,}|~{3,})[^`\\n]*\\n")
  .replace("list", " {0,3}(?:[*+-]|1[.)]) ") // only lists starting from 1 can interrupt
  .replace("html", "</?(?:tag)(?: +|\\n|/?>)|<(?:script|pre|style|!--)")
  .replace("tag", blockTag) // pars can be interrupted by type (6) html blocks
  .getRegex();

block.blockquote = edit(block.blockquote)
  .replace("paragraph", block.paragraph)
  .getRegex();

/**
 * Normal Block Grammar
 */
const blockNormal = Object.assign({}, block);

/**
 * GFM Block Grammar
 */
const blockGfm = Object.assign({}, blockNormal, {
  nptable: /^ *([^|\n ].*\|.*)\n *([-:]+ *\|[-| :]*)(?:\n((?:.*[^>\n ].*(?:\n|$))*)\n*|$)/,
  table: /^ *\|(.+)\n *\|?( *[-:]+[-| :]*)(?:\n((?: *[^>\n ].*(?:\n|$))*)\n*|$)/
});

/**
 * Pedantic grammar (original John Gruber's loose markdown specification)
 */
const blockPedantic = Object.assign({}, blockNormal, {
  html: edit(
    "^ *(?:comment *(?:\\n|\\s*$)" +
    "|<(tag)[\\s\\S]+?</\\1> *(?:\\n{2,}|\\s*$)" + // closed tag
      "|<tag(?:\"[^\"]*\"|'[^']*'|\\s[^'\"/>\\s]*)*?/?> *(?:\\n{2,}|\\s*$))"
  )
    .replace("comment", block._comment)
    .replace(
      /tag/g,
      "(?!(?:" +
        "a|em|strong|small|s|cite|q|dfn|abbr|data|time|code|var|samp|kbd|sub" +
        "|sup|i|b|u|mark|ruby|rt|rp|bdi|bdo|span|br|wbr|ins|del|img)" +
        "\\b)\\w+(?!:|[^\\w\\s@]*@)\\b"
    )
    .getRegex(),
  def: /^ *\[([^\]]+)\]: *<?([^\s>]+)>?(?: +(["(][^\n]+[")]))? *(?:\n+|$)/,
  heading: /^ *(#{1,6}) *([^\n]+?) *(?:#+ *)?(?:\n+|$)/,
  fences: noop, // fences not supported
  paragraph: edit(blockNormal._paragraph)
    .replace("hr", block.hr)
    .replace("heading", " *#{1,6} *[^\n]")
    .replace("lheading", block.lheading)
    .replace("blockquote", " {0,3}>")
    .replace("|fences", "")
    .replace("|list", "")
    .replace("|html", "")
    .getRegex()
});

/**
 * Block Lexer
 */
export class Lexer {
  tokens = createTokens();
  rules = blockNormal;

  constructor(public options = defaults) {
    if (this.options.pedantic) {
      this.rules = blockPedantic;
    } else if (this.options.gfm) {
      this.rules = blockGfm;
    }
  }

  /**
   * Preprocessing
   */
  lex(src: string): Tokens {
    src = src.replace(/\r\n|\r/g, "\n").replace(/\t/g, "    ");

    return this.token(src, true);
  }

  /**
   * Lexing
   */
  token(src: string, top: boolean): Tokens {
    src = src.replace(/^ +$/gm, "");
    let cap: RegExpExecArray | RegExpMatchArray | string | null;

    while (src) {
      // newline
      if ((cap = this.rules.newline.exec(src))) {
        src = src.substring(cap[0].length);
        if (cap[0].length > 1) {
          this.tokens.push({
            type: "space"
          });
        }
      }

      // code
      if ((cap = this.rules.code.exec(src))) {
        const lastToken = this.tokens[this.tokens.length - 1];
        src = src.substring(cap[0].length);
        // An indented code block cannot interrupt a paragraph.
        if (lastToken && lastToken.type === "paragraph") {
          lastToken.text += "\n" + cap[0].trimRight();
        } else {
          cap = cap[0].replace(/^ {4}/gm, "");
          this.tokens.push({
            type: "code",
            codeBlockStyle: "indented",
            text: !this.options.pedantic ? rtrim(cap, "\n") : cap
          });
        }
        continue;
      }

      // fences
      if ((cap = this.rules.fences.exec(src))) {
        src = src.substring(cap[0].length);
        this.tokens.push({
          type: "code",
          lang: cap[2] ? cap[2].trim() : cap[2],
          text: cap[3] || ""
        });
        continue;
      }

      // heading
      if ((cap = this.rules.heading.exec(src))) {
        src = src.substring(cap[0].length);
        this.tokens.push({
          type: "heading",
          depth: cap[1].length,
          text: cap[2]
        });
        continue;
      }

      // table no leading pipe (gfm)
      if ((cap = this.rules.nptable.exec(src))) {
        const header = splitCells(cap[1].replace(/^ *| *\| *$/g, ""));
        const tempAlign = cap[2].replace(/^ *|\| *$/g, "").split(/ *\| */);
        const tempCells = cap[3] ? cap[3].replace(/\n$/, "").split("\n") : [];

        const align: Array<"left" | "right" | "center" | null> = [];
        const cells: string[][] = [];

        if (header.length === tempAlign.length) {
          src = src.substring(cap[0].length);

          for (let i = 0; i < tempAlign.length; i++) {
            if (/^ *-+: *$/.test(tempAlign[i])) {
              align[i] = "right";
            } else if (/^ *:-+: *$/.test(tempAlign[i])) {
              align[i] = "center";
            } else if (/^ *:-+ *$/.test(tempAlign[i])) {
              align[i] = "left";
            } else {
              align[i] = null;
            }
          }

          for (let i = 0; i < tempCells.length; i++) {
            cells[i] = splitCells(tempCells[i], header.length);
          }

          this.tokens.push({
            type: "table",
            header,
            align,
            cells
          });

          continue;
        }
      }

      // hr
      if ((cap = this.rules.hr.exec(src))) {
        src = src.substring(cap[0].length);
        this.tokens.push({
          type: "hr"
        });
        continue;
      }

      // blockquote
      if ((cap = this.rules.blockquote.exec(src))) {
        src = src.substring(cap[0].length);

        this.tokens.push({
          type: "blockquote_start"
        });

        cap = cap[0].replace(/^ *> ?/gm, "");

        // Pass `top` to keep the current
        // "toplevel" state. This is exactly
        // how markdown.pl works.
        this.token(cap, top);

        this.tokens.push({
          type: "blockquote_end"
        });

        continue;
      }

      // list
      if ((cap = this.rules.list.exec(src))) {
        src = src.substring(cap[0].length);
        const bull = cap[2];
        const isordered = bull.length > 1;

        const listStart = {
          type: "list_start" as const,
          ordered: isordered,
          start: isordered ? +bull : undefined,
          loose: false
        };

        this.tokens.push(listStart);

        // Get each top-level item.
        cap = cap[0].match(this.rules.item);
        assert(cap);

        const listItems: ListItemStart[] = [];
        let next = false;
        let i = 0;

        for (; i < cap.length; i++) {
          let item = cap[i];

          // Remove the list item's bullet
          // so it is seen as the next token.
          let space = item.length;
          item = item.replace(/^ *([*+-]|\d+\.) */, "");

          // Outdent whatever the
          // list item contains. Hacky.
          if (~item.indexOf("\n ")) {
            space -= item.length;
            item = !this.options.pedantic
              ? item.replace(new RegExp(`^ {1,${space}}`, "gm"), "")
              : item.replace(/^ {1,4}/gm, "");
          }

          // Determine whether the next list item belongs here.
          // Backpedal if it does not belong in this list.
          if (i !== cap.length - 1) {
            const b = block.bullet.exec(cap[i + 1])![0];
            if (
              bull.length > 1
                ? b.length === 1
                : b.length > 1 || (this.options.smartLists && b !== bull)
            ) {
              src = cap.slice(i + 1).join("\n") + src;
              i = cap.length - 1;
            }
          }

          // Determine whether item is loose or not.
          // Use: /(^|\n)(?! )[^\n]+\n\n(?!\s*$)/
          // for discount behavior.
          let loose = next || /\n\n(?!\s*$)/.test(item);
          if (i !== cap.length - 1) {
            next = item.charAt(item.length - 1) === "\n";
            if (!loose) {
              loose = next;
            }
          }

          if (loose) {
            listStart.loose = true;
          }

          // Check for task list items
          const task = /^\[[ xX]\] /.test(item);
          let checked: boolean | undefined = false;
          if (task) {
            checked = item[1] !== " ";
            item = item.replace(/^\[[ xX]\] +/, "");
          }

          const t = {
            type: "list_item_start" as const,
            task,
            checked,
            loose
          };

          listItems.push(t);
          this.tokens.push(t);

          // Recurse.
          this.token(item, false);

          this.tokens.push({
            type: "list_item_end"
          });
        }

        if (listStart.loose) {
          for (let i = 0; i < listItems.length; i++) {
            listItems[i].loose = true;
          }
        }

        this.tokens.push({
          type: "list_end"
        });

        continue;
      }

      // html
      if ((cap = this.rules.html.exec(src))) {
        src = src.substring(cap[0].length);
        this.tokens.push({
          type: "html",
          text: cap[0]
        });
        continue;
      }

      // def
      if (top && (cap = this.rules.def.exec(src))) {
        src = src.substring(cap[0].length);
        if (cap[3]) cap[3] = cap[3].substring(1, cap[3].length - 1);
        const tag = cap[1].toLowerCase().replace(/\s+/g, " ");
        if (!this.tokens.links[tag]) {
          this.tokens.links[tag] = {
            href: cap[2],
            title: cap[3]
          };
        }
        continue;
      }

      // table (gfm)
      if ((cap = this.rules.table.exec(src))) {
        const header = splitCells(cap[1].replace(/^ *| *\| *$/g, ""));
        const tempAlign = cap[2].replace(/^ *|\| *$/g, "").split(/ *\| */);
        const tempCells = cap[3] ? cap[3].replace(/\n$/, "").split("\n") : [];

        const align: Array<"left" | "right" | "center" | null> = [];
        const cells: string[][] = [];

        if (header.length === tempAlign.length) {
          src = src.substring(cap[0].length);

          for (let i = 0; i < tempAlign.length; i++) {
            if (/^ *-+: *$/.test(tempAlign[i])) {
              align[i] = "right";
            } else if (/^ *:-+: *$/.test(tempAlign[i])) {
              align[i] = "center";
            } else if (/^ *:-+ *$/.test(tempAlign[i])) {
              align[i] = "left";
            } else {
              align[i] = null;
            }
          }

          for (let i = 0; i < tempCells.length; i++) {
            cells[i] = splitCells(
              tempCells[i].replace(/^ *\| *| *\| *$/g, ""),
              header.length
            );
          }

          this.tokens.push({
            type: "table",
            header,
            align,
            cells
          });

          continue;
        }
      }

      // lheading
      if ((cap = this.rules.lheading.exec(src))) {
        src = src.substring(cap[0].length);
        this.tokens.push({
          type: "heading",
          depth: cap[2].charAt(0) === "=" ? 1 : 2,
          text: cap[1]
        });
        continue;
      }

      // top-level paragraph
      if (top && (cap = this.rules.paragraph.exec(src))) {
        src = src.substring(cap[0].length);
        this.tokens.push({
          type: "paragraph",
          text:
            cap[1].charAt(cap[1].length - 1) === "\n"
              ? cap[1].slice(0, -1)
              : cap[1]
        });
        continue;
      }

      // text
      if ((cap = this.rules.text.exec(src))) {
        // Top-level should never reach here.
        src = src.substring(cap[0].length);
        this.tokens.push({
          type: "text",
          text: cap[0]
        });
        continue;
      }

      if (src) {
        throw new Error(`Infinite loop on byte: ${src.charCodeAt(0)}`);
      }
    }

    return this.tokens;
  }
  /**
   * Expose Block Rules
   */
  static rules = block;

  /**
   * Static Lex Method
   */
  static lex(src: string, options?: MarkdownOptions): Tokens {
    const lexer = new Lexer(options);
    return lexer.lex(src);
  }
}

/**
 * Inline-Level Grammar
 */
const inline: Record<string, RegExp> = {
  escape: /^\\([!"#$%&'()*+,\-./:;<=>?@\[\]\\^_`{|}~])/,
  autolink: /^<(scheme:[^\s\x00-\x1f<>]*|email)>/,
  url: noop,
  link: /^!?\[(label)\]\(\s*(href)(?:\s+(title))?\s*\)/,
  reflink: /^!?\[(label)\]\[(?!\s*\])((?:\\[\[\]]?|[^\[\]\\])+)\]/,
  nolink: /^!?\[(?!\s*\])((?:\[[^\[\]]*\]|\\[\[\]]|[^\[\]])*)\](?:\[\])?/,
  strong: /^__([^\s_])__(?!_)|^\*\*([^\s*])\*\*(?!\*)|^__([^\s][\s\S]*?[^\s])__(?!_)|^\*\*([^\s][\s\S]*?[^\s])\*\*(?!\*)/,
  em: /^_([^\s_])_(?!_)|^\*([^\s*<\[])\*(?!\*)|^_([^\s<][\s\S]*?[^\s_])_(?!_|[^\spunctuation])|^_([^\s_<][\s\S]*?[^\s])_(?!_|[^\spunctuation])|^\*([^\s<"][\s\S]*?[^\s\*])\*(?!\*|[^\spunctuation])|^\*([^\s*"<\[][\s\S]*?[^\s])\*(?!\*)/,
  code: /^(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)/,
  br: /^( {2,}|\\)\n(?!\s*$)/,
  del: noop,
  text: /^(`+|[^`])(?:[\s\S]*?(?:(?=[\\<!\[`*]|\b_|$)|[^ ](?= {2,}\n))|(?= {2,}\n))/
};

const inlineTag =
  "^comment" +
  "|^</[a-zA-Z][\\w:-]*\\s*>" + // self-closing tag
  "|^<[a-zA-Z][\\w-]*(?:attribute)*?\\s*/?>" + // open tag
  "|^<\\?[\\s\\S]*?\\?>" + // processing instruction, e.g. <?php ?>
  "|^<![a-zA-Z]+\\s[\\s\\S]*?>" + // declaration, e.g. <!DOCTYPE html>
  "|^<!\\[CDATA\\[[\\s\\S]*?\\]\\]>"; // CDATA section

// list of punctuation marks from common mark spec
// without ` and ] to workaround Rule 17 (inline code blocks/links)
const inlinePunctuation = "!\"#$%&'()*+,\\-./:;<=>?@\\[^_{|}~";
inline.em = edit(inline.em)
  .replace(/punctuation/g, inlinePunctuation)
  .getRegex();

inline._escapes = /\\([!"#$%&'()*+,\-./:;<=>?@\[\]\\^_`{|}~])/g;

inline._scheme = /[a-zA-Z][a-zA-Z0-9+.-]{1,31}/;
inline._email = /[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+(@)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+(?![-_])/;
inline.autolink = edit(inline.autolink)
  .replace("scheme", inline._scheme)
  .replace("email", inline._email)
  .getRegex();

inline._attribute = /\s+[a-zA-Z:_][\w.:-]*(?:\s*=\s*"[^"]*"|\s*=\s*'[^']*'|\s*=\s*[^\s"'=<>`]+)?/;

inline.tag = edit(inlineTag)
  .replace("comment", block._comment)
  .replace("attribute", inline._attribute)
  .getRegex();

inline._label = /(?:\[[^\[\]]*\]|\\.|`[^`]*`|[^\[\]\\`])*?/;
inline._href = /<(?:\\[<>]?|[^\s<>\\])*>|[^\s\x00-\x1f]*/;
inline._title = /"(?:\\"?|[^"\\])*"|'(?:\\'?|[^'\\])*'|\((?:\\\)?|[^)\\])*\)/;

inline.link = edit(inline.link)
  .replace("label", inline._label)
  .replace("href", inline._href)
  .replace("title", inline._title)
  .getRegex();

inline.reflink = edit(inline.reflink)
  .replace("label", inline._label)
  .getRegex();

/**
 * Normal Inline Grammar
 */

const inlineNormal = Object.assign({}, inline);

/**
 * Pedantic Inline Grammar
 */

const inlinePedantic = Object.assign({}, inlineNormal, {
  strong: /^__(?=\S)([\s\S]*?\S)__(?!_)|^\*\*(?=\S)([\s\S]*?\S)\*\*(?!\*)/,
  em: /^_(?=\S)([\s\S]*?\S)_(?!_)|^\*(?=\S)([\s\S]*?\S)\*(?!\*)/,
  link: edit(/^!?\[(label)\]\((.*?)\)/)
    .replace("label", inline._label)
    .getRegex(),
  reflink: edit(/^!?\[(label)\]\s*\[([^\]]*)\]/)
    .replace("label", inline._label)
    .getRegex()
});

/**
 * GFM Inline Grammar
 */

const inlineGfm = Object.assign({}, inlineNormal, {
  escape: edit(inline.escape)
    .replace("])", "~|])")
    .getRegex(),
  _extendedEmail: /[A-Za-z0-9._+-]+(@)[a-zA-Z0-9-_]+(?:\.[a-zA-Z0-9-_]*[a-zA-Z0-9])+(?![-_])/,
  url: /^((?:ftp|https?):\/\/|www\.)(?:[a-zA-Z0-9\-]+\.?)+[^\s<]*|^email/,
  _backpedal: /(?:[^?!.,:;*_~()&]+|\([^)]*\)|&(?![a-zA-Z0-9]+;$)|[?!.,:;*_~)]+(?!$))+/,
  del: /^~+(?=\S)([\s\S]*?\S)~+/,
  text: /^(`+|[^`])(?:[\s\S]*?(?:(?=[\\<!\[`*~]|\b_|https?:\/\/|ftp:\/\/|www\.|$)|[^ ](?= {2,}\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@))|(?= {2,}\n|[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@))/
});

inlineGfm.url = edit(inlineGfm.url, "i")
  .replace("email", inlineGfm._extendedEmail)
  .getRegex();

/**
 * GFM + Line Breaks Inline Grammar
 */
const inlineBreaks = Object.assign({}, inlineGfm, {
  br: edit(inline.br)
    .replace("{2,}", "*")
    .getRegex(),
  text: edit(inlineGfm.text)
    .replace("\\b_", "\\b_| {2,}\\n")
    .replace(/\{2,\}/g, "*")
    .getRegex()
});

/**
 * Inline Lexer & Compiler
 */
export class InlineLexer {
  rules = inlineNormal;
  renderer: Renderer;
  inLink = false;
  inRawBlock = false;

  constructor(public links: Record<string, Link>, public options = defaults) {
    this.renderer = this.options.renderer ?? new Renderer();
    this.renderer.options = this.options;

    if (this.options.pedantic) {
      this.rules = inlinePedantic;
    } else if (this.options.gfm) {
      if (this.options.breaks) {
        this.rules = inlineBreaks;
      } else {
        this.rules = inlineGfm;
      }
    }
  }

  /**
   * Lexing/Compiling
   */
  output(src: string): string {
    let out = "";
    let cap: RegExpMatchArray | null;

    while (src) {
      // escape
      if ((cap = this.rules.escape.exec(src))) {
        src = src.substring(cap[0].length);
        out += escape(cap[1]);
        continue;
      }

      // tag
      if ((cap = this.rules.tag.exec(src))) {
        if (!this.inLink && /^<a /i.test(cap[0])) {
          this.inLink = true;
        } else if (this.inLink && /^<\/a>/i.test(cap[0])) {
          this.inLink = false;
        }
        if (!this.inRawBlock && /^<(pre|code|kbd|script)(\s|>)/i.test(cap[0])) {
          this.inRawBlock = true;
        } else if (
          this.inRawBlock &&
          /^<\/(pre|code|kbd|script)(\s|>)/i.test(cap[0])
        ) {
          this.inRawBlock = false;
        }

        src = src.substring(cap[0].length);
        out += cap[0];
        continue;
      }

      // link
      if ((cap = this.rules.link.exec(src))) {
        const lastParenIndex = findClosingBracket(cap[2], "()");
        if (lastParenIndex > -1) {
          const start = cap[0].indexOf("!") === 0 ? 5 : 4;
          const linkLen = start + cap[1].length + lastParenIndex;
          cap[2] = cap[2].substring(0, lastParenIndex);
          cap[0] = cap[0].substring(0, linkLen).trim();
          cap[3] = "";
        }
        src = src.substring(cap[0].length);
        this.inLink = true;
        let href = cap[2];
        let title: string;
        if (this.options.pedantic) {
          const link = /^([^'"]*[^\s])\s+(['"])(.*)\2/.exec(href);

          if (link) {
            href = link[1];
            title = link[3];
          } else {
            title = "";
          }
        } else {
          title = cap[3] ? cap[3].slice(1, -1) : "";
        }
        href = href.trim().replace(/^<([\s\S]*)>$/, "$1");
        out += this.outputLink(cap, {
          href: InlineLexer.escapes(href),
          title: InlineLexer.escapes(title)
        });
        this.inLink = false;
        continue;
      }

      // reflink, nolink
      if (
        (cap = this.rules.reflink.exec(src)) ||
        (cap = this.rules.nolink.exec(src))
      ) {
        src = src.substring(cap[0].length);
        const linkId = (cap[2] || cap[1]).replace(/\s+/g, " ");
        const link = this.links[Number(linkId.toLowerCase())];
        if (!link || !link.href) {
          out += cap[0].charAt(0);
          src = cap[0].substring(1) + src;
          continue;
        }
        this.inLink = true;
        out += this.outputLink(cap, link);
        this.inLink = false;
        continue;
      }

      // strong
      if ((cap = this.rules.strong.exec(src))) {
        src = src.substring(cap[0].length);
        out += this.renderer.strong(
          this.output(cap[4] || cap[3] || cap[2] || cap[1])
        );
        continue;
      }

      // em
      if ((cap = this.rules.em.exec(src))) {
        src = src.substring(cap[0].length);
        out += this.renderer.em(
          this.output(cap[6] || cap[5] || cap[4] || cap[3] || cap[2] || cap[1])
        );
        continue;
      }

      // code
      if ((cap = this.rules.code.exec(src))) {
        src = src.substring(cap[0].length);
        out += this.renderer.codespan(escape(cap[2].trim(), true));
        continue;
      }

      // br
      if ((cap = this.rules.br.exec(src))) {
        src = src.substring(cap[0].length);
        out += this.renderer.br();
        continue;
      }

      // del (gfm)
      if ((cap = this.rules.del.exec(src))) {
        src = src.substring(cap[0].length);
        out += this.renderer.del(this.output(cap[1]));
        continue;
      }

      // autolink
      if ((cap = this.rules.autolink.exec(src))) {
        src = src.substring(cap[0].length);
        let text: string;
        let href: string;
        if (cap[2] === "@") {
          text = escape(this.mangle(cap[1]));
          href = `mailto:${text}`;
        } else {
          text = escape(cap[1]);
          href = text;
        }
        out += this.renderer.link(href, null, text);
        continue;
      }

      // url (gfm)
      if (!this.inLink && (cap = this.rules.url.exec(src))) {
        let text: string;
        let href: string;
        if (cap[2] === "@") {
          text = escape(cap[0]);
          href = `mailto:${text}`;
        } else {
          // do extended autolink path validation
          let prevCapZero: string;
          do {
            prevCapZero = cap[0];
            cap[0] = this.rules._backpedal.exec(cap[0])![0];
          } while (prevCapZero !== cap[0]);
          text = escape(cap[0]);
          if (cap[1] === "www.") {
            href = `http://${text}`;
          } else {
            href = text;
          }
        }
        src = src.substring(cap[0].length);
        out += this.renderer.link(href, null, text);
        continue;
      }

      // text
      if ((cap = this.rules.text.exec(src))) {
        src = src.substring(cap[0].length);
        if (this.inRawBlock) {
          out += this.renderer.text(cap[0]);
        } else {
          out += this.renderer.text(escape(this.smartypants(cap[0])));
        }
        continue;
      }

      if (src) {
        throw new Error(`Infinite loop on byte: ${src.charCodeAt(0)}`);
      }
    }

    return out;
  }

  /**
   * Compile Link
   */
  outputLink(cap: RegExpMatchArray, link: Link): string {
    const href = link.href;
    const title = link.title ? escape(link.title) : null;

    return cap[0].charAt(0) !== "!"
      ? this.renderer.link(href, title, this.output(cap[1]))
      : this.renderer.image(href, title, escape(cap[1]));
  }

  /**
   * Smartypants Transformations
   */
  smartypants(text: string): string {
    if (!this.options.smartypants) return text;
    return (
      text
        // em-dashes
        .replace(/---/g, "\u2014")
        // en-dashes
        .replace(/--/g, "\u2013")
        // opening singles
        .replace(/(^|[-\u2014/(\[{"\s])'/g, "$1\u2018")
        // closing singles & apostrophes
        .replace(/'/g, "\u2019")
        // opening doubles
        .replace(/(^|[-\u2014/(\[{\u2018\s])"/g, "$1\u201c")
        // closing doubles
        .replace(/"/g, "\u201d")
        // ellipses
        .replace(/\.{3}/g, "\u2026")
    );
  }

  /**
   * Mangle Links
   */
  mangle(text: string): string {
    if (!this.options.mangle) {
      return text;
    }
    let out = "";

    for (let i = 0; i < text.length; i++) {
      let ch: number | string = text.charCodeAt(i);
      if (Math.random() > 0.5) {
        ch = `x${ch.toString(16)}`;
      }
      out += `&#${ch};`;
    }

    return out;
  }

  /**
   * Expose Inline Rules
   */
  static rules = inline;

  /**
   * Static Lexing/Compiling Method
   */
  static output(
    src: string,
    links: Record<string, Link>,
    options?: MarkdownOptions
  ): string {
    const inline = new InlineLexer(links, options);
    return inline.output(src);
  }

  static escapes(text: string): string {
    return text ? text.replace(InlineLexer.rules._escapes, "$1") : text;
  }
}

/**
 * Renderer
 */
export class Renderer {
  constructor(public options = defaults) {}

  code(code: string, infostring = "", escaped = false): string {
    const lang = infostring.match(/\S*/)![0];
    if (this.options.highlight) {
      const out = this.options.highlight(code, lang);
      if (out != null && out !== code) {
        escaped = true;
        code = out;
      }
    }

    if (!lang) {
      return `<pre><code>${escaped ? code : escape(code, true)}</code></pre>`;
    }

    return `<pre><code class="${this.options.langPrefix}${escape(
      lang,
      true
    )}">${escaped ? code : escape(code, true)}</code></pre>\n`;
  }

  blockquote(quote: string): string {
    return `<blockquote>\n${quote}</blockquote>\n`;
  }

  html(html: string): string {
    return html;
  }

  heading(text: string, level: number, raw: string, slugger: Slugger): string {
    if (this.options.headerIds) {
      return `<h${level} id="${this.options.headerPrefix}${slugger.slug(
        raw
      )}">${text}</h${level}>\n`;
    }
    // ignore IDs
    return `<h${level}>${text}</h${level}>\n`;
  }

  hr(): string {
    return this.options.xhtml ? "<hr/>\n" : "<hr>\n";
  }

  list(body: string, ordered: boolean, start?: number): string {
    const type = ordered ? "ol" : "ul";
    const startatt = ordered && start !== 1 ? ` start="${start}"` : "";
    return `<${type}${startatt}>\n${body}</${type}>\n`;
  }

  listitem(text: string, _task: boolean, _checked?: boolean): string {
    return `<li>${text}</li>\n`;
  }

  checkbox(checked?: boolean): string {
    return `<input ${checked ? 'checked="" ' : ""}disabled="" type="checkbox"${
      this.options.xhtml ? " /" : ""
    }> `;
  }

  paragraph(text: string): string {
    return `<p>${text}</p>\n`;
  }

  table(header: string, body: string): string {
    if (body) {
      body = `<tbody>${body}</tbody>`;
    }

    return `<table>\n<thead>\n${header}</thead>\n${body}</table>\n`;
  }

  tablerow(content: string): string {
    return `<tr>\n${content}</tr>\n`;
  }

  tablecell(content: string, flags: TableCellFlags): string {
    const type = flags.header ? "th" : "td";
    const tag = flags.align ? `<${type} align="${flags.align}">` : `<${type}>`;
    return `${tag}${content}</${type}>\n`;
  }

  // span level renderer
  strong(text: string): string {
    return `<strong>${text}</strong>`;
  }

  em(text: string): string {
    return `<em>${text}</em>`;
  }

  codespan(text: string): string {
    return `<code>${text}</code>`;
  }

  br(): string {
    return this.options.xhtml ? "<br/>" : "<br>";
  }

  del(text: string): string {
    return `<del>${text}</del>`;
  }

  link(href: string, title: string | null, text: string): string {
    const cleanHref = cleanUrl(this.options.baseUrl, href);
    if (cleanHref === null) {
      return text;
    }
    let out = `<a href="${escape(cleanHref)}"`;
    if (title) {
      out += ` title="${title}"`;
    }
    out += `>${text}</a>`;
    return out;
  }

  image(href: string, title: string | null, text: string): string {
    const cleanHref = cleanUrl(this.options.baseUrl, href);
    if (cleanHref === null) {
      return text;
    }

    let out = `<img src="${cleanHref}" alt="${text}"`;
    if (title) {
      out += ` title="${title}"`;
    }
    out += this.options.xhtml ? "/>" : ">";
    return out;
  }

  text(text: string): string {
    return text;
  }
}

/**
 * TextRenderer
 * returns only the textual part of the token
 */
export class TextRenderer {
  strong(text: string): string {
    return text;
  }

  em(text: string): string {
    return text;
  }

  codespan(text: string): string {
    return text;
  }

  del(text: string): string {
    return text;
  }

  text(text: string): string {
    return text;
  }

  link(_href: string, _title: string, text: string): string {
    return String(text);
  }

  image(_href: string, _title: string, text: string): string {
    return String(text);
  }

  br(): string {
    return "";
  }
}

/**
 * Parsing & Compiling
 */
export class Parser {
  inline?: InlineLexer;
  inlineText?: InlineLexer;
  tokens: Token[] = [];
  token?: Token | null = null;
  renderer: Renderer;
  slugger = new Slugger();

  constructor(public options = defaults) {
    this.options.renderer = this.options.renderer || new Renderer();
    this.renderer = this.options.renderer;
    this.renderer.options = this.options;
  }

  /**
   * Parse Loop
   */
  parse(src: Tokens): string {
    this.inline = new InlineLexer(src.links, this.options);
    // use an InlineLexer with a TextRenderer to extract pure text
    this.inlineText = new InlineLexer(
      src.links,
      Object.assign({}, this.options, { renderer: new TextRenderer() })
    );
    this.tokens = src.reverse();

    let out = "";
    while (this.next()) {
      out += this.tok();
    }

    return out;
  }

  /**
   * Next Token
   */
  next(): Token | undefined {
    this.token = this.tokens.pop();
    return this.token;
  }

  /**
   * Preview Next Token
   */
  peek(): Token | undefined {
    return this.tokens[this.tokens.length - 1] || undefined;
  }

  /**
   * Parse Text Tokens
   */
  parseText(): string {
    assert(this.token);
    assert("text" in this.token);
    let body = this.token.text;

    while (this.peek()!.type === "text") {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      body += "\n" + (this.next() as any).text;
    }

    return this.inline!.output(body);
  }

  /**
   * Parse Current Token
   */
  tok(): string | undefined {
    assert(this.token);
    assert(this.inline);
    assert(this.inlineText);
    switch (this.token.type) {
      case "space": {
        return "";
      }
      case "hr": {
        return this.renderer.hr();
      }
      case "heading": {
        return this.renderer.heading(
          this.inline.output(this.token.text),
          this.token.depth,
          unescape(this.inlineText.output(this.token.text)),
          this.slugger
        );
      }
      case "code": {
        return this.renderer.code(
          this.token.text,
          this.token.lang,
          this.token.escaped
        );
      }
      case "table": {
        // header
        let cell = "";
        for (let i = 0; i < this.token.header.length; i++) {
          cell += this.renderer.tablecell(
            this.inline.output(this.token.header[i]),
            { header: true, align: this.token.align[i] }
          );
        }
        const header = this.renderer.tablerow(cell);

        let body = "";
        for (let i = 0; i < this.token.cells.length; i++) {
          const row = this.token.cells[i];

          cell = "";
          for (let j = 0; j < row.length; j++) {
            cell += this.renderer.tablecell(this.inline.output(row[j]), {
              header: false,
              align: this.token.align[j]
            });
          }

          body += this.renderer.tablerow(cell);
        }
        return this.renderer.table(header, body);
      }
      case "blockquote_start": {
        let body = "";

        while (this.next()!.type !== "blockquote_end") {
          body += this.tok();
        }

        return this.renderer.blockquote(body);
      }
      case "list_start": {
        let body = "";
        const { ordered, start } = this.token;

        while (this.next()!.type !== "list_end") {
          body += this.tok();
        }

        return this.renderer.list(body, ordered, start);
      }
      case "list_item_start": {
        let body = "";
        const { loose, checked, task } = this.token;

        if (this.token.task) {
          if (loose) {
            if (this.peek()!.type === "text") {
              const nextToken = this.peek()!;
              assert("text" in nextToken);
              nextToken.text =
                this.renderer.checkbox(checked) + " " + nextToken.text;
            } else {
              this.tokens.push({
                type: "text",
                text: this.renderer.checkbox(checked)
              });
            }
          } else {
            body += this.renderer.checkbox(checked);
          }
        }

        while (this.next()!.type !== "list_item_end") {
          body +=
            !loose && (this.token as Token).type === "text"
              ? this.parseText()
              : this.tok();
        }
        return this.renderer.listitem(body, task, checked);
      }
      case "html": {
        // TODO parse inline content if parameter markdown=1
        return this.renderer.html(this.token.text);
      }
      case "paragraph": {
        return this.renderer.paragraph(this.inline.output(this.token.text));
      }
      case "text": {
        return this.renderer.paragraph(this.parseText());
      }
      default: {
        const errMsg = `Token with "${this.token.type}" type was not found.`;
        if (this.options.silent) {
          console.log(errMsg);
        } else {
          throw new Error(errMsg);
        }
      }
    }
  }

  /**
   * Static Parse Method
   */
  static parse(src: Tokens, options?: MarkdownOptions): string {
    const parser = new Parser(options);
    return parser.parse(src);
  }
}

/**
 * Slugger generates header id
 */

export class Slugger {
  seen: Record<string, number> = {};

  /**
   * Convert string to unique id
   */
  slug(value: string): string {
    let slug = value
      .toLowerCase()
      .trim()
      .replace(
        /[\u2000-\u206F\u2E00-\u2E7F\\'!"#$%&()*+,./:;<=>?@[\]^`{|}~]/g,
        ""
      )
      .replace(/\s/g, "-");

    if (this.seen.hasOwnProperty(slug)) {
      const originalSlug = slug;
      do {
        this.seen[originalSlug]++;
        slug = `${originalSlug}-${this.seen[originalSlug]}`;
      } while (this.seen.hasOwnProperty(slug));
    }
    this.seen[slug] = 0;

    return slug;
  }
}

/**
 * Helpers
 */

function escape(html: string, encode = false): string {
  if (encode) {
    if (escapeTest.test(html)) {
      return html.replace(escapeReplace, ch => escapeReplacements[ch]);
    }
  } else {
    if (escapeTestNoEncode.test(html)) {
      return html.replace(escapeReplaceNoEncode, ch => escapeReplacements[ch]);
    }
  }

  return html;
}

const escapeTest = /[&<>"']/;
const escapeReplace = /[&<>"']/g;
const escapeReplacements = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;"
} as Record<string, string>;

const escapeTestNoEncode = /[<>"']|&(?!#?\w+;)/;
const escapeReplaceNoEncode = /[<>"']|&(?!#?\w+;)/g;

function unescape(html: string): string {
  // explicitly match decimal, hex, and named HTML entities
  return html.replace(
    /&(#(?:\d+)|(?:#x[0-9A-Fa-f]+)|(?:\w+));?/gi,
    (_, n: string) => {
      n = n.toLowerCase();
      if (n === "colon") {
        return ":";
      }
      if (n.charAt(0) === "#") {
        return n.charAt(1) === "x"
          ? String.fromCharCode(parseInt(n.substring(2), 16))
          : String.fromCharCode(+n.substring(1));
      }
      return "";
    }
  );
}

function getString(val: RegExp | string): string {
  return typeof val === "string" ? val : val.source;
}

function edit(regex: RegExp | string, opt = ""): EditChain {
  let result = getString(regex);
  return {
    replace(name, val: RegExp | string): EditChain {
      val = getString(val);
      val = val.replace(/(^|[^\[])\^/g, "$1");
      result = result.replace(name, val);
      return this;
    },
    getRegex(): RegExp {
      return new RegExp(result, opt);
    }
  };
}

function cleanUrl(base: string | null, href: string): string | null {
  if (base && !originIndependentUrl.test(href)) {
    href = resolveUrl(base, href);
  }
  try {
    href = encodeURI(href).replace(/%25/g, "%");
  } catch (e) {
    return null;
  }
  return href;
}

const baseUrls: Record<string, string> = {};
const originIndependentUrl = /^$|^[a-z][a-z0-9+.-]*:|^[?#]/i;

function resolveUrl(base: string, href: string): string {
  if (!baseUrls[` ${base}`]) {
    // we can ignore everything in base after the last slash of its path component,
    // but we might need to add _that_
    // https://tools.ietf.org/html/rfc3986#section-3
    if (/^[^:]+:\/*[^/]*$/.test(base)) {
      baseUrls[` ${base}`] = base + "/";
    } else {
      baseUrls[` ${base}`] = rtrim(base, "/", true);
    }
  }
  base = baseUrls[` ${base}`];
  const relativeBase = base.indexOf(":") === -1;

  if (href.slice(0, 2) === "//") {
    if (relativeBase) {
      return href;
    }
    return base.replace(/^([^:]+:)[\s\S]*$/, "$1") + href;
  } else if (href.charAt(0) === "/") {
    if (relativeBase) {
      return href;
    }
    return base.replace(/^([^:]+:\/*[^/]*)[\s\S]*$/, "$1") + href;
  } else {
    return `${base}${href}`;
  }
}

function splitCells(tableRow: string, count?: number): string[] {
  // ensure that every cell-delimiting pipe has a space
  // before it to distinguish it from an escaped pipe
  const row = tableRow.replace(/\|/g, (_match, offset, str) => {
    let escaped = false;
    let curr = offset;
    while (--curr >= 0 && str[curr] === "\\") escaped = !escaped;
    if (escaped) {
      // odd number of slashes means | is escaped
      // so we leave it alone
      return "|";
    } else {
      // add space before unescaped |
      return " |";
    }
  });
  const cells = row.split(/ \|/);
  let i = 0;

  if (cells.length > count!) {
    cells.splice(count!);
  } else {
    while (cells.length < count!) {
      cells.push("");
    }
  }

  for (; i < cells.length; i++) {
    // leading or trailing whitespace is ignored per the gfm spec
    cells[i] = cells[i].trim().replace(/\\\|/g, "|");
  }
  return cells;
}

/** Remove trailing `c`s. Equivalent to `str.replace(/c*$/, "")`.
 *
 * `/c*$/` is vulnerable to REDOS.
 *
 * @param invert Remove suffix of non-`c` chars instead. Default `false`.
 */
function rtrim(str: string, c: string, invert = false): string {
  if (str.length === 0) {
    return "";
  }

  // Length of suffix matching the invert condition.
  let suffLen = 0;

  // Step left until we fail to match the invert condition.
  while (suffLen < str.length) {
    const currChar = str.charAt(str.length - suffLen - 1);
    if (currChar === c && !invert) {
      suffLen++;
    } else if (currChar !== c && invert) {
      suffLen++;
    } else {
      break;
    }
  }

  return str.substr(0, str.length - suffLen);
}

function findClosingBracket(str: string, b: string): number {
  if (str.indexOf(b[1]) === -1) {
    return -1;
  }
  let level = 0;
  for (let i = 0; i < str.length; i++) {
    if (str[i] === "\\") {
      i++;
    } else if (str[i] === b[0]) {
      level++;
    } else if (str[i] === b[1]) {
      level--;
      if (level < 0) {
        return i;
      }
    }
  }
  return -1;
}

/**
 * Core function which takes an input of markdown and returns HTML.
 */
export function parse(
  src: string,
  opt?: Partial<MarkdownOptions>
): string | void {
  try {
    const options: MarkdownOptions = Object.assign({}, defaults, opt);
    return Parser.parse(Lexer.lex(src, options), options);
  } catch (e) {
    if ((opt || defaults).silent) {
      return `<p>An error occurred:</p><pre>${escape(
        e.message + "",
        true
      )}</pre>`;
    }
    throw e;
  }
}

/**
 * Options
 */

export const options = (opt: Partial<MarkdownOptions>): void => {
  Object.assign(defaults, opt);
};

export const setOptions = options;

export const getDefaults = (): MarkdownOptions => {
  const opt = {
    baseUrl: null,
    breaks: false,
    gfm: true,
    headerIds: true,
    headerPrefix: "",
    highlight: null,
    langPrefix: "language-",
    mangle: true,
    pedantic: false,
    silent: false,
    smartLists: false,
    smartypants: false,
    xhtml: false
  } as MarkdownOptions;
  opt.renderer = new Renderer(opt);
  return opt;
};

export const defaults = getDefaults();

export const parser = Parser.parse;
export const lexer = Lexer.lex;
export const inlineLexer = InlineLexer.output;

export default parse;
