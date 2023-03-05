// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#![allow(unused)]

// Alphabet chars.
pub const CHAR_UPPERCASE_A: u32 = 65; /* A */
pub const CHAR_LOWERCASE_A: u32 = 97; /* a */
pub const CHAR_UPPERCASE_Z: u32 = 90; /* Z */
pub const CHAR_LOWERCASE_Z: u32 = 122; /* z */

// Non-alphabetic chars.
pub const CHAR_DOT: u32 = 46; /* . */
pub const CHAR_FORWARD_SLASH: u32 = 47; /* / */
pub const CHAR_BACKWARD_SLASH: u32 = 92; /* \ */
pub const CHAR_VERTICAL_LINE: u32 = 124; /* | */
pub const CHAR_COLON: u32 = 58; /* : */
pub const CHAR_QUESTION_MARK: u32 = 63; /* ? */
pub const CHAR_UNDERSCORE: u32 = 95; /* _ */
pub const CHAR_LINE_FEED: u32 = 10; /* \n */
pub const CHAR_CARRIAGE_RETURN: u32 = 13; /* \r */
pub const CHAR_TAB: u32 = 9; /* \t */
pub const CHAR_FORM_FEED: u32 = 12; /* \f */
pub const CHAR_EXCLAMATION_MARK: u32 = 33; /* ! */
pub const CHAR_HASH: u32 = 35; /* # */
pub const CHAR_SPACE: u32 = 32; /*   */
pub const CHAR_NO_BREAK_SPACE: u32 = 160; /* \u00A0 */
pub const CHAR_ZERO_WIDTH_NOBREAK_SPACE: u32 = 65279; /* \uFEFF */
pub const CHAR_LEFT_SQUARE_BRACKET: u32 = 91; /* [ */
pub const CHAR_RIGHT_SQUARE_BRACKET: u32 = 93; /* ] */
pub const CHAR_LEFT_ANGLE_BRACKET: u32 = 60; /* < */
pub const CHAR_RIGHT_ANGLE_BRACKET: u32 = 62; /* > */
pub const CHAR_LEFT_CURLY_BRACKET: u32 = 123; /* { */
pub const CHAR_RIGHT_CURLY_BRACKET: u32 = 125; /* } */
pub const CHAR_HYPHEN_MINUS: u32 = 45; /* - */
pub const CHAR_PLUS: u32 = 43; /* + */
pub const CHAR_DOUBLE_QUOTE: u32 = 34; /* " */
pub const CHAR_SINGLE_QUOTE: u32 = 39; /* ' */
pub const CHAR_PERCENT: u32 = 37; /* % */
pub const CHAR_SEMICOLON: u32 = 59; /* ; */
pub const CHAR_CIRCUMFLEX_ACCENT: u32 = 94; /* ^ */
pub const CHAR_GRAVE_ACCENT: u32 = 96; /* ` */
pub const CHAR_AT: u32 = 64; /* @ */
pub const CHAR_AMPERSAND: u32 = 38; /* & */
pub const CHAR_EQUAL: u32 = 61; /* = */

// Digits
pub const CHAR_0: u32 = 48; /* 0 */
pub const CHAR_9: u32 = 57; /* 9 */
