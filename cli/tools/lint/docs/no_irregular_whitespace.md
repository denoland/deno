Disallows the use of non-space or non-tab whitespace characters

Non-space or non-tab whitespace characters can be very difficult to spot in your
code as editors will often render them invisibly. These invisible characters can
cause issues or unexpected behaviors. Sometimes these characters are added
inadvertently through copy/paste or incorrect keyboard shortcuts.

The following characters are disallowed:

```
\u000B - Line Tabulation (\v) - <VT>
\u000C - Form Feed (\f) - <FF>
\u00A0 - No-Break Space - <NBSP>
\u0085 - Next Line
\u1680 - Ogham Space Mark
\u180E - Mongolian Vowel Separator - <MVS>
\ufeff - Zero Width No-Break Space - <BOM>
\u2000 - En Quad
\u2001 - Em Quad
\u2002 - En Space - <ENSP>
\u2003 - Em Space - <EMSP>
\u2004 - Tree-Per-Em
\u2005 - Four-Per-Em
\u2006 - Six-Per-Em
\u2007 - Figure Space
\u2008 - Punctuation Space - <PUNCSP>
\u2009 - Thin Space
\u200A - Hair Space
\u200B - Zero Width Space - <ZWSP>
\u2028 - Line Separator
\u2029 - Paragraph Separator
\u202F - Narrow No-Break Space
\u205f - Medium Mathematical Space
\u3000 - Ideographic Space
```

To fix this linting issue, replace instances of the above with regular spaces,
tabs or new lines. If it's not obvious where the offending character(s) are try
retyping the line from scratch.
