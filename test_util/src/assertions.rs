#[macro_export]
macro_rules! assert_ends_with {
  ($left:expr, $right:expr $(,)?) => {
    match (&$left, &$right) {
      (actual, expected) => {
        let actual = if expected.len() > actual.len() {
          actual
        } else {
          &actual[actual.len() - expected.len()..]
        };
        pretty_assertions::assert_eq!(
          actual,
          *expected,
          "should end with expected."
        );
      }
    }
  };
}

#[macro_export]
macro_rules! assert_contains {
  ($string:expr, $($test:expr),+ $(,)?) => {
    let string = &$string; // This might be a function call or something
    if !($(string.contains($test))||+) {
      panic!("{:?} does not contain any of {:?}", string, [$($test),+]);
    }
  }
}

#[macro_export]
macro_rules! assert_not_contains {
  ($string:expr, $($test:expr),+ $(,)?) => {
    let string = &$string; // This might be a function call or something
    if !($(!string.contains($test))||+) {
      panic!("{:?} contained {:?}", string, [$($test),+]);
    }
  }
}
