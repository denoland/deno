// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-fmt-ignore-file
(function () {
  const { core } = globalThis.__bootstrap;
  const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");

  // Alphabet chars.
  const CHAR_UPPERCASE_A = 65; /* A */
  const CHAR_LOWERCASE_A = 97; /* a */
  const CHAR_UPPERCASE_Z = 90; /* Z */
  const CHAR_LOWERCASE_Z = 122; /* z */
  const CHAR_UPPERCASE_C = 67; /* C */
  const CHAR_LOWERCASE_B = 98; /* b */
  const CHAR_LOWERCASE_E = 101; /* e */
  const CHAR_LOWERCASE_N = 110; /* n */

  // Non-alphabetic chars.
  const CHAR_DOT = 46; /* . */
  const CHAR_FORWARD_SLASH = 47; /* / */
  const CHAR_BACKWARD_SLASH = 92; /* \ */
  const CHAR_VERTICAL_LINE = 124; /* | */
  const CHAR_COLON = 58; /* = */
  const CHAR_QUESTION_MARK = 63; /* ? */
  const CHAR_UNDERSCORE = 95; /* _ */
  const CHAR_LINE_FEED = 10; /* \n */
  const CHAR_CARRIAGE_RETURN = 13; /* \r */
  const CHAR_TAB = 9; /* \t */
  const CHAR_FORM_FEED = 12; /* \f */
  const CHAR_EXCLAMATION_MARK = 33; /* ! */
  const CHAR_HASH = 35; /* # */
  const CHAR_SPACE = 32; /*   */
  const CHAR_NO_BREAK_SPACE = 160; /* \u00A0 */
  const CHAR_ZERO_WIDTH_NOBREAK_SPACE = 65279; /* \uFEFF */
  const CHAR_LEFT_SQUARE_BRACKET = 91; /* [ */
  const CHAR_RIGHT_SQUARE_BRACKET = 93; /* ] */
  const CHAR_LEFT_ANGLE_BRACKET = 60; /* < */
  const CHAR_RIGHT_ANGLE_BRACKET = 62; /* > */
  const CHAR_LEFT_CURLY_BRACKET = 123; /* { */
  const CHAR_RIGHT_CURLY_BRACKET = 125; /* } */
  const CHAR_HYPHEN_MINUS = 45; /* - */
  const CHAR_PLUS = 43; /* + */
  const CHAR_DOUBLE_QUOTE = 34; /* " */
  const CHAR_SINGLE_QUOTE = 39; /* ' */
  const CHAR_PERCENT = 37; /* % */
  const CHAR_SEMICOLON = 59; /* ; */
  const CHAR_CIRCUMFLEX_ACCENT = 94; /* ^ */
  const CHAR_GRAVE_ACCENT = 96; /* ` */
  const CHAR_AT = 64; /* @ */
  const CHAR_AMPERSAND = 38; /* & */
  const CHAR_EQUAL = 61; /* = */

  // Digits
  const CHAR_0 = 48; /* 0 */
  const CHAR_9 = 57; /* 9 */

  const EOL = isWindows ? "\r\n" : "\n";

  const _defaultExport = {
    CHAR_UPPERCASE_A,
    CHAR_LOWERCASE_A,
    CHAR_UPPERCASE_Z,
    CHAR_LOWERCASE_Z,
    CHAR_UPPERCASE_C,
    CHAR_LOWERCASE_B,
    CHAR_LOWERCASE_E,
    CHAR_LOWERCASE_N,
    CHAR_DOT,
    CHAR_FORWARD_SLASH,
    CHAR_BACKWARD_SLASH,
    CHAR_VERTICAL_LINE,
    CHAR_COLON,
    CHAR_QUESTION_MARK,
    CHAR_UNDERSCORE,
    CHAR_LINE_FEED,
    CHAR_CARRIAGE_RETURN,
    CHAR_TAB,
    CHAR_FORM_FEED,
    CHAR_EXCLAMATION_MARK,
    CHAR_HASH,
    CHAR_SPACE,
    CHAR_NO_BREAK_SPACE,
    CHAR_ZERO_WIDTH_NOBREAK_SPACE,
    CHAR_LEFT_SQUARE_BRACKET,
    CHAR_RIGHT_SQUARE_BRACKET,
    CHAR_LEFT_ANGLE_BRACKET,
    CHAR_RIGHT_ANGLE_BRACKET,
    CHAR_LEFT_CURLY_BRACKET,
    CHAR_RIGHT_CURLY_BRACKET,
    CHAR_HYPHEN_MINUS,
    CHAR_PLUS,
    CHAR_DOUBLE_QUOTE,
    CHAR_SINGLE_QUOTE,
    CHAR_PERCENT,
    CHAR_SEMICOLON,
    CHAR_CIRCUMFLEX_ACCENT,
    CHAR_GRAVE_ACCENT,
    CHAR_AT,
    CHAR_AMPERSAND,
    CHAR_EQUAL,

    CHAR_0,
    CHAR_9,

    EOL,
  };

  return {
    CHAR_UPPERCASE_A,
    CHAR_LOWERCASE_A,
    CHAR_UPPERCASE_Z,
    CHAR_LOWERCASE_Z,
    CHAR_UPPERCASE_C,
    CHAR_LOWERCASE_B,
    CHAR_LOWERCASE_E,
    CHAR_LOWERCASE_N,
    CHAR_DOT,
    CHAR_FORWARD_SLASH,
    CHAR_BACKWARD_SLASH,
    CHAR_VERTICAL_LINE,
    CHAR_COLON,
    CHAR_QUESTION_MARK,
    CHAR_UNDERSCORE,
    CHAR_LINE_FEED,
    CHAR_CARRIAGE_RETURN,
    CHAR_TAB,
    CHAR_FORM_FEED,
    CHAR_EXCLAMATION_MARK,
    CHAR_HASH,
    CHAR_SPACE,
    CHAR_NO_BREAK_SPACE,
    CHAR_ZERO_WIDTH_NOBREAK_SPACE,
    CHAR_LEFT_SQUARE_BRACKET,
    CHAR_RIGHT_SQUARE_BRACKET,
    CHAR_LEFT_ANGLE_BRACKET,
    CHAR_RIGHT_ANGLE_BRACKET,
    CHAR_LEFT_CURLY_BRACKET,
    CHAR_RIGHT_CURLY_BRACKET,
    CHAR_HYPHEN_MINUS,
    CHAR_PLUS,
    CHAR_DOUBLE_QUOTE,
    CHAR_SINGLE_QUOTE,
    CHAR_PERCENT,
    CHAR_SEMICOLON,
    CHAR_CIRCUMFLEX_ACCENT,
    CHAR_GRAVE_ACCENT,
    CHAR_AT,
    CHAR_AMPERSAND,
    CHAR_EQUAL,
    CHAR_0,
    CHAR_9,
    EOL,
    default: _defaultExport,
  };
})()
