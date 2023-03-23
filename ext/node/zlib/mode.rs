macro_rules! repr_i32 {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
      $($(#[$vmeta:meta])* $vname:ident $(= $val:expr)?,)*
    }) => {
      $(#[$meta])*
      $vis enum $name {
        $($(#[$vmeta])* $vname $(= $val)?,)*
      }

      #[derive(Debug)]
      pub enum Error {
        InvalidMode,
      }

      impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          match self {
            Error::InvalidMode => write!(f, "invalid mode"),
          }
        }
      }

      impl std::error::Error for Error {}

      impl core::convert::TryFrom<i32> for $name {
        type Error = Error;

        fn try_from(v: i32) -> Result<Self, Self::Error> {
          match v {
            $(x if x == $name::$vname as i32 => Ok($name::$vname),)*
            _ => Err(Error::InvalidMode),
          }
        }
      }
    }
  }

repr_i32! {
  #[repr(i32)]
  #[derive(Debug, PartialEq)]
  pub enum Mode {
    None,
    Deflate,
    Inflate,
    Gzip,
    Gunzip,
    DeflateRaw,
    InflateRaw,
    Unzip,
  }
}
