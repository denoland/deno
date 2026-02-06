use std::iter::Peekable;

pub struct ChangeAndOutput {
  pub change: SpecSection,
  pub output: SpecSection,
}

pub struct ConfigChangeSpec {
  pub original_text: SpecSection,
  pub change_and_outputs: Vec<ChangeAndOutput>,
}

impl ConfigChangeSpec {
  pub fn parse(text: &str) -> Self {
    let mut sections = SpecSection::parse_many(text);
    let original_text = sections.remove(0);
    let mut change_and_outputs = Vec::new();
    let mut sections = sections.into_iter().peekable();
    while sections.peek().is_some() {
      change_and_outputs.push(ChangeAndOutput {
        change: sections.next().unwrap(),
        output: sections.next().unwrap(),
      });
    }
    Self {
      original_text,
      change_and_outputs,
    }
  }

  pub fn emit(&self) -> String {
    let mut text = String::new();
    text.push_str(&self.original_text.emit());
    for (i, change_and_output) in self.change_and_outputs.iter().enumerate() {
      if i > 0 {
        text.push('\n');
      }
      text.push_str(&change_and_output.change.emit());
      text.push_str(&change_and_output.output.emit());
    }
    text
  }
}

pub struct SpecSection {
  pub title: String,
  pub text: String,
}

impl SpecSection {
  pub fn parse_many(text: &str) -> Vec<Self> {
    fn take_header<'a>(lines: &mut impl Iterator<Item = &'a str>) -> String {
      lines
        .next()
        .unwrap()
        .strip_prefix("# ")
        .unwrap()
        .to_string()
    }

    fn take_next<'a>(
      lines: &mut Peekable<impl Iterator<Item = &'a str>>,
    ) -> String {
      let mut result = String::new();
      while let Some(line) = lines.next() {
        result.push_str(line);
        result.push('\n');
        if let Some(next_line) = lines.peek()
          && next_line.starts_with('#')
        {
          break;
        }
      }
      result
    }

    let mut lines = text.split('\n').peekable();
    let mut sections = Vec::new();
    while lines.peek().is_some() {
      sections.push(SpecSection {
        title: take_header(&mut lines),
        text: take_next(&mut lines),
      });
    }
    sections
  }

  pub fn emit(&self) -> String {
    format!("# {}\n{}", self.title, self.text)
  }
}
