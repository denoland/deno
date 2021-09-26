use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct Error {
  source: io::Error,
  op_name: String,
  path: PathBuf,
}

impl Error {
  pub fn new(
    source: io::Error,
    op_name: &str,
    path: impl Into<PathBuf>,
  ) -> io::Error {
    Self::_new(source, op_name, path.into())
  }

  fn _new(source: io::Error, op_name: &str, path: PathBuf) -> io::Error {
    io::Error::new(
      source.kind(),
      Self {
        source,
        op_name: op_name.to_string(),
        path,
      },
    )
  }
}

impl fmt::Display for Error {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let path = self.path.display();

    write!(formatter, "{}, {} '{}'", self.source, self.op_name, path)
  }
}

impl StdError for Error {
  fn cause(&self) -> Option<&dyn StdError> {
    self.source()
  }

  fn source(&self) -> Option<&(dyn StdError + 'static)> {
    Some(&self.source)
  }
}

#[derive(Debug)]
pub(crate) struct SourceDestError {
  source: io::Error,
  op_name: String,
  from_path: PathBuf,
  to_path: PathBuf,
}

impl SourceDestError {
  pub fn new(
    source: io::Error,
    op_name: &str,
    from_path: impl Into<PathBuf>,
    to_path: impl Into<PathBuf>,
  ) -> io::Error {
    Self::_new(source, op_name, from_path.into(), to_path.into())
  }

  fn _new(
    source: io::Error,
    op_name: &str,
    from_path: PathBuf,
    to_path: PathBuf,
  ) -> io::Error {
    io::Error::new(
      source.kind(),
      Self {
        source,
        op_name: op_name.to_string(),
        from_path,
        to_path,
      },
    )
  }
}

impl fmt::Display for SourceDestError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let from = self.from_path.display();
    let to = self.to_path.display();

    write!(
      formatter,
      "{}, {} '{}' -> '{}'",
      self.source, self.op_name, from, to
    )
  }
}

impl StdError for SourceDestError {
  fn cause(&self) -> Option<&dyn StdError> {
    self.source()
  }

  fn source(&self) -> Option<&(dyn StdError + 'static)> {
    Some(&self.source)
  }
}
