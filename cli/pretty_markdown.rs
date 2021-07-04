// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::highlight;
use crate::media_type::MediaType;

/// Generate ANSI-colored string based on the given markdown
pub fn colorize(md: impl AsRef<str>) -> String {
  let md_tokens = markdown::tokenize(md.as_ref());
  md_tokens.colorize()
}

trait Colorize {
  fn colorize(self) -> String;
}

impl Colorize for markdown::Block {
  fn colorize(self) -> String {
    use markdown::Block::*;
    match self {
      Header(spans, 1) => {
        colors::magenta_bold_underline_italic(spans.colorize())
          .to_string()
          .linebreak()
      }
      Header(spans, 2 | 3) => colors::magenta_bold_underline(spans.colorize())
        .to_string()
        .linebreak(),
      Header(spans, 4) => colors::magenta_bold(spans.colorize())
        .to_string()
        .linebreak(),
      Header(spans, _level) => {
        colors::magenta(spans.colorize()).to_string().linebreak()
      }
      Paragraph(spans) => spans.colorize().linebreak(),
      Blockquote(blocks) => {
        colors::gray(blocks.colorize()).to_string().linebreak()
      }
      CodeBlock(Some(info), content) => {
        let media_type = parse_info_string(info);

        if matches!(
          media_type,
          MediaType::TypeScript
            | MediaType::JavaScript
            | MediaType::Jsx
            | MediaType::Tsx
            | MediaType::Dts
        ) {
          let mut v = Vec::new();

          for line in content.split('\n') {
            let highlighted = highlight::highlight_line(line, &media_type);
            v.push(highlighted.indent(4));
          }

          v.join("\n").linebreak()
        } else {
          content
            .split('\n')
            .map(|line| line.indent(4))
            .join_by("\n")
            .linebreak()
        }
      }
      CodeBlock(None, content) => content
        .split('\n')
        .map(|line| line.indent(4))
        .join_by("\n")
        .linebreak(),
      OrderedList(list_items, _list_type) => list_items
        .into_iter()
        .enumerate()
        .map(|(idx, li)| format!("{}. {}", idx, li.colorize()).indent(2))
        .join_by("\n")
        .linebreak(),
      UnorderedList(list_items) => list_items
        .into_iter()
        .map(|li| format!("• {}", li.colorize()).indent(2))
        .join_by("\n")
        .linebreak(),
      Raw(content) => content.linebreak(),
      Hr => colors::gray("-".repeat(80)).to_string().linebreak(),
    }
  }
}

impl Colorize for Vec<markdown::Block> {
  fn colorize(self) -> String {
    self.into_iter().map(Colorize::colorize).join_by("\n")
  }
}

impl Colorize for markdown::Span {
  fn colorize(self) -> String {
    use markdown::Span::*;
    match self {
      Break => "\n".to_string(),
      Text(text) => text,
      Code(code) => colors::green(code).to_string(),
      Link(label, url, _title) => {
        format!("[{label}]({url})", label = label, url = url)
      }
      Image(alt, url, _title) => {
        format!("![{alt}]({url})", alt = alt, url = url)
      }
      Emphasis(spans) => colors::italic(spans.colorize()).to_string(),
      Strong(spans) => colors::bold(spans.colorize()).to_string(),
    }
  }
}

impl Colorize for Vec<markdown::Span> {
  fn colorize(self) -> String {
    self.into_iter().map(Colorize::colorize).join_by("")
  }
}

impl Colorize for markdown::ListItem {
  fn colorize(self) -> String {
    use markdown::ListItem::*;
    match self {
      Simple(spans) => spans.colorize(),
      Paragraph(blocks) => blocks.colorize(),
    }
  }
}

trait Linebreak {
  fn linebreak(self) -> String;
}

impl Linebreak for String {
  fn linebreak(self) -> String {
    format!("{}\n", self)
  }
}

trait Indent {
  fn indent(self, width: usize) -> String;
}

impl Indent for String {
  fn indent(self, width: usize) -> String {
    format!("{}{}", " ".repeat(width), self)
  }
}

impl Indent for &str {
  fn indent(self, width: usize) -> String {
    format!("{}{}", " ".repeat(width), self)
  }
}

trait JoinBy {
  fn join_by(self, sep: &str) -> String;
}

impl<I> JoinBy for I
where
  I: Iterator<Item = String>,
{
  fn join_by(self, sep: &str) -> String {
    self.collect::<Vec<_>>().join(sep)
  }
}

/// Parse a info string of a fenced code block in markdown to determine what language is used in
/// the block
fn parse_info_string(info_string: String) -> MediaType {
  if let Some(lang) = info_string.trim().split_whitespace().next() {
    match lang.to_ascii_lowercase().as_str() {
      "js" | "javascript" => MediaType::JavaScript,
      "ts" | "typescript" => MediaType::TypeScript,
      "jsx" => MediaType::Jsx,
      "tsx" => MediaType::Tsx,
      "dts" => MediaType::Dts,
      _ => MediaType::Unknown,
    }
  } else {
    MediaType::Unknown
  }
}
