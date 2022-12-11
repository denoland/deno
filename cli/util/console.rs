use deno_runtime::ops::tty::ConsoleSize;

/// Gets the console size.
pub fn console_size() -> Option<ConsoleSize> {
  let stderr = &deno_runtime::ops::io::STDERR_HANDLE;
  deno_runtime::ops::tty::console_size(stderr).ok()
}

pub fn show_cursor() {
  eprint!("\x1B[?25h");
}

pub fn hide_cursor() {
  eprint!("\x1B[?25l");
}

const VTS_MOVE_TO_ZERO_COL: &str = "\x1B[0G";
const VTS_CLEAR_CURRENT_LINE: &str = "\x1B[2K";
const VTS_CLEAR_CURSOR_DOWN: &str = "\x1B[J";
const VTS_CLEAR_UNTIL_NEWLINE: &str = "\x1B[K";

fn vts_move_up(count: usize) -> String {
  if count == 0 {
    String::new()
  } else {
    format!("\x1B[{}A", count)
  }
}

#[derive(Debug, Default)]
pub struct StaticConsoleText {
  last_lines: Vec<line_rendering::Line>,
  last_terminal_width: u32,
}

impl StaticConsoleText {
  pub fn clear(&mut self, terminal_width: u32) {
    let last_lines = self.get_last_lines(terminal_width);
    if !last_lines.is_empty() {
      eprint!(
        "{}{}{}",
        VTS_MOVE_TO_ZERO_COL,
        vts_move_up(last_lines.len()),
        VTS_CLEAR_CURSOR_DOWN
      );
    }
  }

  pub fn set(&mut self, new_text: &str, terminal_width: u32) {
    let is_terminal_width_different =
      terminal_width != self.last_terminal_width;
    let last_lines = self.get_last_lines(terminal_width);
    let new_lines =
      line_rendering::render_text_to_lines(new_text, terminal_width);
    if !are_collections_equal(&last_lines, &new_lines) {
      let mut text = String::new();
      text.push_str(VTS_MOVE_TO_ZERO_COL);
      if last_lines.len() > 0 {
        text.push_str(&vts_move_up(last_lines.len() - 1));
      }
      if is_terminal_width_different {
        text.push_str(VTS_CLEAR_CURSOR_DOWN);
      }
      for i in 0..std::cmp::max(last_lines.len(), new_lines.len()) {
        let last_line = last_lines.get(i);
        let new_line = new_lines.get(i);
        if i > 0 {
          text.push_str("\n");
        }
        if let Some(new_line) = new_line {
          text.push_str(&new_line.text);
          if let Some(last_line) = last_line {
            if last_line.char_width > new_line.char_width {
              text.push_str(VTS_CLEAR_UNTIL_NEWLINE);
            }
          }
        } else {
          text.push_str(VTS_CLEAR_CURRENT_LINE);
        }
      }
      if last_lines.len() > new_lines.len() {
        text.push_str(&vts_move_up(last_lines.len() - new_lines.len()));
      }
      eprint!("{}", text);
    }
    self.last_lines = new_lines;
    self.last_terminal_width = terminal_width;
  }

  fn get_last_lines(
    &mut self,
    terminal_width: u32,
  ) -> Vec<line_rendering::Line> {
    // render based on how the text looks right now
    let terminal_width = if self.last_terminal_width < terminal_width {
      self.last_terminal_width
    } else {
      terminal_width
    };

    if terminal_width == self.last_terminal_width {
      self.last_lines.drain(..).collect()
    } else {
      // render the last text with the current terminal width
      let line_texts = self
        .last_lines
        .drain(..)
        .map(|l| l.text)
        .collect::<Vec<_>>();
      line_rendering::render_text_to_lines(
        &line_texts.join("\n"),
        terminal_width,
      )
    }
  }
}

fn are_collections_equal<T: PartialEq>(a: &[T], b: &[T]) -> bool {
  a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a == b)
}

/// The code in this module was lifted from dprint with some changes.
/// Copyright 2020-2022 David Sherret - MIT License
mod line_rendering {
  #[derive(Debug, PartialEq, Eq)]
  pub struct Line {
    pub char_width: u32,
    pub text: String,
  }

  pub fn render_text_to_lines(text: &str, terminal_width: u32) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut line_width: u32 = 0;
    let mut current_whitespace = String::new();
    for token in tokenize_words(text) {
      match token {
        WordToken::Word(word, word_width) => {
          let is_word_longer_than_line = word_width > terminal_width;
          if is_word_longer_than_line {
            // break it up onto multiple lines with indentation
            if !current_whitespace.is_empty() {
              if line_width < terminal_width {
                current_line.push_str(&current_whitespace);
              }
              current_whitespace = String::new();
            }
            for c in word.chars() {
              if line_width == terminal_width {
                lines.push(Line {
                  char_width: line_width,
                  text: current_line,
                });
                current_line = String::new();
                line_width = 0;
              }
              current_line.push(c);
              line_width += 1;
            }
          } else {
            if line_width + word_width > terminal_width {
              lines.push(Line {
                char_width: line_width,
                text: current_line,
              });
              current_line = String::new();
              line_width = 0;
              current_whitespace = String::new();
            }
            if !current_whitespace.is_empty() {
              current_line.push_str(&current_whitespace);
              current_whitespace = String::new();
            }
            current_line.push_str(word);
            line_width += word_width;
          }
        }
        WordToken::WhiteSpace(space_char) => {
          current_whitespace.push(space_char);
          line_width += 1;
        }
        WordToken::NewLine => {
          lines.push(Line {
            char_width: line_width,
            text: current_line,
          });
          current_line = String::new();
          line_width = 0;
        }
      }
    }
    if !current_line.is_empty() {
      lines.push(Line {
        char_width: line_width,
        text: current_line,
      });
    }
    lines
  }

  enum WordToken<'a> {
    Word(&'a str, u32),
    WhiteSpace(char),
    NewLine,
  }

  impl<'a> WordToken<'a> {
    pub fn word(text: &'a str) -> Self {
      WordToken::Word(
        text,
        strip_ansi_escapes::strip(&text)
          .ok()
          .and_then(|bytes| String::from_utf8(bytes).ok())
          .unwrap_or_else(|| text.to_string())
          .chars()
          .count() as u32,
      )
    }
  }

  fn tokenize_words(text: &str) -> Vec<WordToken> {
    let mut start_index = 0;
    let mut tokens = Vec::new();
    for (index, c) in text.char_indices() {
      if c.is_whitespace() || c == '\n' {
        let new_word_text = &text[start_index..index];
        if new_word_text.len() > 0 {
          tokens.push(WordToken::word(new_word_text));
        }

        if c == '\n' {
          tokens.push(WordToken::NewLine);
        } else {
          tokens.push(WordToken::WhiteSpace(c));
        }

        start_index = index + c.len_utf8(); // start at next char
      }
    }

    let new_word_text = &text[start_index..];
    if new_word_text.len() > 0 {
      tokens.push(WordToken::word(new_word_text));
    }
    tokens
  }
}
