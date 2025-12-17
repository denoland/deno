// Copyright 2018-2025 the Deno authors. MIT license.

use crate::colors;

pub enum WildcardMatchResult {
  Success,
  Fail(String),
}

impl WildcardMatchResult {
  pub fn is_success(&self) -> bool {
    matches!(self, WildcardMatchResult::Success)
  }
}

pub fn wildcard_match_detailed(
  pattern: &str,
  text: &str,
) -> WildcardMatchResult {
  fn annotate_whitespace(text: &str) -> String {
    text.replace('\t', "\u{2192}").replace(' ', "\u{00B7}")
  }

  // Normalize line endings
  let original_text = text.replace("\r\n", "\n");
  let mut current_text = original_text.as_str();
  // normalize line endings and strip comments
  let pattern = pattern
    .split('\n')
    .map(|line| line.trim_end_matches('\r'))
    .filter(|l| {
      let is_comment = l.starts_with("[#") && l.ends_with(']');
      !is_comment
    })
    .collect::<Vec<_>>()
    .join("\n");
  let mut output_lines = Vec::new();

  let parts = parse_wildcard_pattern_text(&pattern).unwrap();

  let mut was_last_wildcard = false;
  let mut was_last_wildline = false;
  for (i, part) in parts.iter().enumerate() {
    match part {
      WildcardPatternPart::Wildcard => {
        output_lines.push("<WILDCARD />".to_string());
      }
      WildcardPatternPart::Wildline => {
        output_lines.push("<WILDLINE />".to_string());
      }
      WildcardPatternPart::Wildnum(times) => {
        if current_text.len() < *times {
          output_lines
            .push(format!("==== HAD MISSING WILDCHARS({}) ====", times));
          output_lines.push(colors::red(annotate_whitespace(current_text)));
          return WildcardMatchResult::Fail(output_lines.join("\n"));
        }
        output_lines.push(format!("<WILDCHARS({}) />", times));
        current_text = &current_text[*times..];
      }
      WildcardPatternPart::Text(search_text) => {
        let is_last = i + 1 == parts.len();
        let search_index = if is_last && was_last_wildcard {
          // search from the end of the file
          current_text.rfind(search_text)
        } else if was_last_wildline {
          if is_last {
            find_last_text_on_line(search_text, current_text)
          } else {
            find_first_text_on_line(search_text, current_text)
          }
        } else {
          current_text.find(search_text)
        };
        match search_index {
          Some(found_index)
            if was_last_wildcard || was_last_wildline || found_index == 0 =>
          {
            output_lines.push(format!(
              "<FOUND>{}</FOUND>",
              colors::gray(annotate_whitespace(search_text))
            ));
            current_text = &current_text[found_index + search_text.len()..];
          }
          Some(index) => {
            output_lines.push(
              "==== FOUND SEARCH TEXT IN WRONG POSITION ====".to_string(),
            );
            output_lines.push(colors::gray(annotate_whitespace(search_text)));
            output_lines
              .push("==== HAD UNKNOWN PRECEDING TEXT ====".to_string());
            output_lines
              .push(colors::red(annotate_whitespace(&current_text[..index])));
            return WildcardMatchResult::Fail(output_lines.join("\n"));
          }
          None => {
            let was_wildcard_or_line = was_last_wildcard || was_last_wildline;
            let mut max_search_text_found_index = 0;
            let mut max_current_text_found_index = 0;
            for (index, _) in search_text.char_indices() {
              let sub_string = &search_text[..index];
              if let Some(found_index) = current_text.find(sub_string) {
                if was_wildcard_or_line || found_index == 0 {
                  max_search_text_found_index = index;
                  max_current_text_found_index = found_index;
                } else {
                  break;
                }
              } else {
                break;
              }
            }
            if !was_wildcard_or_line && max_search_text_found_index > 0 {
              output_lines.push(format!(
                "<FOUND>{}</FOUND>",
                colors::gray(annotate_whitespace(
                  &search_text[..max_search_text_found_index]
                ))
              ));
            }
            output_lines
              .push("==== COULD NOT FIND SEARCH TEXT ====".to_string());
            output_lines.push(colors::green(annotate_whitespace(
              if was_wildcard_or_line {
                search_text
              } else {
                &search_text[max_search_text_found_index..]
              },
            )));
            if was_wildcard_or_line && max_search_text_found_index > 0 {
              output_lines.push(format!(
                "==== MAX FOUND ====\n{}",
                colors::red(annotate_whitespace(
                  &search_text[..max_search_text_found_index]
                ))
              ));
            }
            let actual_next_text =
              &current_text[max_current_text_found_index..];
            let next_text_len = actual_next_text
              .chars()
              .take(40)
              .map(|c| c.len_utf8())
              .sum::<usize>();
            output_lines.push(format!(
              "==== NEXT ACTUAL TEXT ====\n{}{}",
              colors::red(annotate_whitespace(
                &actual_next_text[..next_text_len]
              )),
              if actual_next_text.len() > next_text_len {
                "[TRUNCATED]"
              } else {
                ""
              },
            ));
            return WildcardMatchResult::Fail(output_lines.join("\n"));
          }
        }
      }
      WildcardPatternPart::UnorderedLines(expected_lines) => {
        assert!(!was_last_wildcard, "unsupported");
        assert!(!was_last_wildline, "unsupported");
        let mut actual_lines = Vec::with_capacity(expected_lines.len());
        for _ in 0..expected_lines.len() {
          match current_text.find('\n') {
            Some(end_line_index) => {
              actual_lines.push(&current_text[..end_line_index]);
              current_text = &current_text[end_line_index + 1..];
            }
            None => {
              break;
            }
          }
        }
        actual_lines.sort_unstable();
        let mut expected_lines = expected_lines.clone();
        expected_lines.sort_unstable();

        if actual_lines.len() != expected_lines.len() {
          output_lines
            .push("==== HAD WRONG NUMBER OF UNORDERED LINES ====".to_string());
          output_lines.push("# ACTUAL".to_string());
          output_lines.extend(
            actual_lines
              .iter()
              .map(|l| colors::green(annotate_whitespace(l))),
          );
          output_lines.push("# EXPECTED".to_string());
          output_lines.extend(
            expected_lines
              .iter()
              .map(|l| colors::green(annotate_whitespace(l))),
          );
          return WildcardMatchResult::Fail(output_lines.join("\n"));
        }

        if let Some(invalid_expected) =
          expected_lines.iter().find(|e| e.contains("[WILDCARD]"))
        {
          panic!(
            concat!(
              "Cannot use [WILDCARD] inside [UNORDERED_START]. Use [WILDLINE] instead.\n",
              "  Invalid expected line: {}"
            ),
            invalid_expected
          );
        }

        for actual_line in actual_lines {
          let maybe_found_index =
            expected_lines.iter().position(|expected_line| {
              actual_line == *expected_line
                || wildcard_match_detailed(expected_line, actual_line)
                  .is_success()
            });
          if let Some(found_index) = maybe_found_index {
            let expected = expected_lines.remove(found_index);
            output_lines.push(format!(
              "<FOUND>{}</FOUND>",
              colors::gray(annotate_whitespace(expected))
            ));
          } else {
            output_lines
              .push("==== UNORDERED LINE DID NOT MATCH ====".to_string());
            output_lines.push(format!(
              "  ACTUAL: {}",
              colors::red(annotate_whitespace(actual_line))
            ));
            for expected in expected_lines {
              output_lines.push(format!(
                "  EXPECTED ANY: {}",
                colors::green(annotate_whitespace(expected))
              ));
            }
            return WildcardMatchResult::Fail(output_lines.join("\n"));
          }
        }
      }
    }
    was_last_wildcard = matches!(part, WildcardPatternPart::Wildcard);
    was_last_wildline = matches!(part, WildcardPatternPart::Wildline);
  }

  if was_last_wildcard || was_last_wildline || current_text.is_empty() {
    WildcardMatchResult::Success
  } else if current_text == "\n" {
    WildcardMatchResult::Fail(
      "<matched everything>\n!!!! PROBLEM: Missing final newline at end of expected output !!!!"
        .to_string(),
    )
  } else {
    output_lines.push("==== HAD TEXT AT END OF FILE ====".to_string());
    output_lines.push(colors::red(annotate_whitespace(current_text)));
    WildcardMatchResult::Fail(output_lines.join("\n"))
  }
}

#[derive(Debug)]
enum WildcardPatternPart<'a> {
  Wildcard,
  Wildline,
  Wildnum(usize),
  Text(&'a str),
  UnorderedLines(Vec<&'a str>),
}

fn parse_wildcard_pattern_text(
  text: &str,
) -> Result<Vec<WildcardPatternPart<'_>>, monch::ParseErrorFailureError> {
  use monch::*;

  fn parse_unordered_lines(input: &str) -> ParseResult<'_, Vec<&str>> {
    const END_TEXT: &str = "\n[UNORDERED_END]\n";
    let (input, _) = tag("[UNORDERED_START]\n")(input)?;
    match input.find(END_TEXT) {
      Some(end_index) => ParseResult::Ok((
        &input[end_index + END_TEXT.len()..],
        input[..end_index].lines().collect::<Vec<_>>(),
      )),
      None => ParseError::fail(input, "Could not find [UNORDERED_END]"),
    }
  }

  enum InnerPart<'a> {
    Wildcard,
    Wildline,
    Wildchars(usize),
    UnorderedLines(Vec<&'a str>),
    Char,
  }

  struct Parser<'a> {
    current_input: &'a str,
    last_text_input: &'a str,
    parts: Vec<WildcardPatternPart<'a>>,
  }

  impl<'a> Parser<'a> {
    fn parse(mut self) -> ParseResult<'a, Vec<WildcardPatternPart<'a>>> {
      fn parse_num(input: &str) -> ParseResult<'_, usize> {
        let num_char_count =
          input.chars().take_while(|c| c.is_ascii_digit()).count();
        if num_char_count == 0 {
          return ParseError::backtrace();
        }
        let (char_text, input) = input.split_at(num_char_count);
        let value = str::parse::<usize>(char_text).unwrap();
        Ok((input, value))
      }

      fn parse_wild_char(input: &str) -> ParseResult<'_, ()> {
        let (input, _) = tag("[WILDCHAR]")(input)?;
        ParseResult::Ok((input, ()))
      }

      fn parse_wild_chars(input: &str) -> ParseResult<'_, usize> {
        let (input, _) = tag("[WILDCHARS(")(input)?;
        let (input, times) = parse_num(input)?;
        let (input, _) = tag(")]")(input)?;
        ParseResult::Ok((input, times))
      }

      while !self.current_input.is_empty() {
        let (next_input, inner_part) = or6(
          map(tag("[WILDCARD]"), |_| InnerPart::Wildcard),
          map(tag("[WILDLINE]"), |_| InnerPart::Wildline),
          map(parse_wild_char, |_| InnerPart::Wildchars(1)),
          map(parse_wild_chars, InnerPart::Wildchars),
          map(parse_unordered_lines, |lines| {
            InnerPart::UnorderedLines(lines)
          }),
          map(next_char, |_| InnerPart::Char),
        )(self.current_input)?;
        match inner_part {
          InnerPart::Wildcard => {
            self.queue_previous_text(next_input);
            self.parts.push(WildcardPatternPart::Wildcard);
          }
          InnerPart::Wildline => {
            self.queue_previous_text(next_input);
            self.parts.push(WildcardPatternPart::Wildline);
          }
          InnerPart::Wildchars(times) => {
            self.queue_previous_text(next_input);
            self.parts.push(WildcardPatternPart::Wildnum(times));
          }
          InnerPart::UnorderedLines(expected_lines) => {
            self.queue_previous_text(next_input);
            self
              .parts
              .push(WildcardPatternPart::UnorderedLines(expected_lines));
          }
          InnerPart::Char => {
            // ignore
          }
        }
        self.current_input = next_input;
      }

      self.queue_previous_text("");

      ParseResult::Ok(("", self.parts))
    }

    fn queue_previous_text(&mut self, next_input: &'a str) {
      let previous_text = &self.last_text_input
        [..self.last_text_input.len() - self.current_input.len()];
      if !previous_text.is_empty() {
        self.parts.push(WildcardPatternPart::Text(previous_text));
      }
      self.last_text_input = next_input;
    }
  }

  with_failure_handling(|input| {
    Parser {
      current_input: input,
      last_text_input: input,
      parts: Vec::new(),
    }
    .parse()
  })(text)
}

fn find_first_text_on_line(
  search_text: &str,
  current_text: &str,
) -> Option<usize> {
  let end_search_pos = current_text.find('\n').unwrap_or(current_text.len());
  let found_pos = current_text.find(search_text)?;
  if found_pos <= end_search_pos {
    Some(found_pos)
  } else {
    None
  }
}

fn find_last_text_on_line(
  search_text: &str,
  current_text: &str,
) -> Option<usize> {
  let end_search_pos = current_text.find('\n').unwrap_or(current_text.len());
  let mut best_match = None;
  let mut search_pos = 0;
  while let Some(new_pos) = current_text[search_pos..].find(search_text) {
    search_pos += new_pos;
    if search_pos <= end_search_pos {
      best_match = Some(search_pos);
    } else {
      break;
    }
    search_pos += 1;
  }
  best_match
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::assert_contains;

  #[test]
  fn parse_parse_wildcard_match_text() {
    let result =
      parse_wildcard_pattern_text("[UNORDERED_START]\ntesting\ntesting")
        .err()
        .unwrap();
    assert_contains!(result.to_string(), "Could not find [UNORDERED_END]");
  }

  #[test]
  fn test_wildcard_match() {
    let fixtures = vec![
      ("foobarbaz", "foobarbaz", true),
      ("[WILDCARD]", "foobarbaz", true),
      ("foobar", "foobarbaz", false),
      ("foo[WILDCARD]baz", "foobarbaz", true),
      ("foo[WILDCARD]baz", "foobazbar", false),
      ("foo[WILDCARD]baz[WILDCARD]qux", "foobarbazqatqux", true),
      ("foo[WILDCARD]", "foobar", true),
      ("foo[WILDCARD]baz[WILDCARD]", "foobarbazqat", true),
      // check with different line endings
      ("foo[WILDCARD]\nbaz[WILDCARD]\n", "foobar\nbazqat\n", true),
      (
        "foo[WILDCARD]\nbaz[WILDCARD]\n",
        "foobar\r\nbazqat\r\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\n",
        "foobar\nbazqat\r\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
        "foobar\nbazqat\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
        "foobar\r\nbazqat\r\n",
        true,
      ),
    ];

    // Iterate through the fixture lists, testing each one
    for (pattern, string, expected) in fixtures {
      let actual = wildcard_match_detailed(pattern, string).is_success();
      dbg!(pattern, string, expected);
      assert_eq!(actual, expected);
    }
  }

  #[test]
  fn test_wildcard_match2() {
    let wildcard_match = |pattern: &str, text: &str| {
      wildcard_match_detailed(pattern, text).is_success()
    };

    // foo, bar, baz, qux, quux, quuz, corge, grault, garply, waldo, fred, plugh, xyzzy
    assert!(wildcard_match("foo[WILDCARD]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCARD]baz", "foobazbar"));

    let multiline_pattern = "[WILDCARD]
foo:
[WILDCARD]baz[WILDCARD]";

    fn multi_line_builder(input: &str, leading_text: Option<&str>) -> String {
      // If there is leading text add a newline so it's on it's own line
      let head = match leading_text {
        Some(v) => format!("{v}\n"),
        None => "".to_string(),
      };
      format!(
        "{head}foo:
quuz {input} corge
grault"
      )
    }

    // Validate multi-line string builder
    assert_eq!(
      "QUUX=qux
foo:
quuz BAZ corge
grault",
      multi_line_builder("BAZ", Some("QUUX=qux"))
    );

    // Correct input & leading line
    assert!(wildcard_match(
      multiline_pattern,
      &multi_line_builder("baz", Some("QUX=quux")),
    ));

    // Should fail when leading line
    assert!(!wildcard_match(
      multiline_pattern,
      &multi_line_builder("baz", None),
    ));

    // Incorrect input & leading line
    assert!(!wildcard_match(
      multiline_pattern,
      &multi_line_builder("garply", Some("QUX=quux")),
    ));

    // Incorrect input & no leading line
    assert!(!wildcard_match(
      multiline_pattern,
      &multi_line_builder("garply", None),
    ));

    // wildline
    assert!(wildcard_match("foo[WILDLINE]baz", "foobarbaz"));
    assert!(wildcard_match("foo[WILDLINE]bar", "foobarbar"));
    assert!(!wildcard_match("foo[WILDLINE]baz", "fooba\nrbaz"));
    assert!(wildcard_match("foo[WILDLINE]", "foobar"));

    // wildnum
    assert!(wildcard_match("foo[WILDCHARS(3)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(4)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(2)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(1)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(20)]baz", "foobarbaz"));
  }

  #[test]
  fn test_wildcard_match_unordered_lines() {
    let wildcard_match = |pattern: &str, text: &str| {
      wildcard_match_detailed(pattern, text).is_success()
    };
    // matching
    assert!(wildcard_match(
      concat!("[UNORDERED_START]\n", "B\n", "A\n", "[UNORDERED_END]\n"),
      concat!("A\n", "B\n",)
    ));
    // different line
    assert!(!wildcard_match(
      concat!("[UNORDERED_START]\n", "Ba\n", "A\n", "[UNORDERED_END]\n"),
      concat!("A\n", "B\n",)
    ));
    // different number of lines
    assert!(!wildcard_match(
      concat!(
        "[UNORDERED_START]\n",
        "B\n",
        "A\n",
        "C\n",
        "[UNORDERED_END]\n"
      ),
      concat!("A\n", "B\n",)
    ));
  }

  #[test]
  fn test_find_first_text_on_line() {
    let text = "foo\nbar\nbaz";
    assert_eq!(find_first_text_on_line("foo", text), Some(0));
    assert_eq!(find_first_text_on_line("oo", text), Some(1));
    assert_eq!(find_first_text_on_line("o", text), Some(1));
    assert_eq!(find_first_text_on_line("o\nbar", text), Some(2));
    assert_eq!(find_first_text_on_line("f", text), Some(0));
    assert_eq!(find_first_text_on_line("bar", text), None);
  }

  #[test]
  fn test_find_last_text_on_line() {
    let text = "foo\nbar\nbaz";
    assert_eq!(find_last_text_on_line("foo", text), Some(0));
    assert_eq!(find_last_text_on_line("oo", text), Some(1));
    assert_eq!(find_last_text_on_line("o", text), Some(2));
    assert_eq!(find_last_text_on_line("o\nbar", text), Some(2));
    assert_eq!(find_last_text_on_line("f", text), Some(0));
    assert_eq!(find_last_text_on_line("bar", text), None);
  }
}
