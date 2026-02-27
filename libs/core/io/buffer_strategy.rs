// Copyright 2018-2025 the Deno authors. MIT license.

/// Our standard buffer size if we don't know what else to do.
const STANDARD_BUFFER_SIZE: usize = 64 * 1024;
/// Our buffer size if the resource is tiny or likely empty.
const TINY_BUFFER_SIZE: usize = 1024;
const MAX_GROW_LEN: usize = 16 * 1024 * 1024;

/// Our size classes.
enum SizeClass {
  // Zero min, no max (ie: no hint given)
  Unknown,
  // Likely empty (min == max == 0)
  LikelyEmpty,
  // Likely known (min == max)
  LikelyKnown(usize),
  // Bounded, but not known
  Between(usize, usize),
  // Bounded maximum, but not known
  BoundedMax(usize),
  // Bounded minimum, but not known
  BoundedMin(usize),
}

impl SizeClass {
  fn from_size_hint(min: usize, maybe_max: Option<usize>) -> Self {
    match (min, maybe_max) {
      (0, None) => Self::Unknown,
      (min, None) => Self::BoundedMin(min),

      (0, Some(0)) => Self::LikelyEmpty,
      (0, Some(max)) => Self::BoundedMax(max),
      (min, Some(max)) if min == max => Self::LikelyKnown(min),
      (min, Some(max)) => Self::Between(min, max),
    }
  }

  #[cfg(test)]
  fn to_size_hint(&self) -> (usize, Option<usize>) {
    match self {
      SizeClass::Unknown => (0, None),
      SizeClass::LikelyEmpty => (0, Some(0)),
      SizeClass::LikelyKnown(n) => (*n, Some(*n)),
      SizeClass::BoundedMin(n) => (*n, None),
      SizeClass::BoundedMax(n) => (0, Some(*n)),
      SizeClass::Between(min, max) => (*min, Some(*max)),
    }
  }

  /// A good guess for an initial buffer size.
  fn initial_buffer_size(&self) -> usize {
    let size = match self {
      SizeClass::Unknown => STANDARD_BUFFER_SIZE,
      SizeClass::LikelyEmpty => 0,
      SizeClass::LikelyKnown(n) => usize::next_power_of_two(*n),
      SizeClass::BoundedMin(n) => {
        std::cmp::max(STANDARD_BUFFER_SIZE, usize::next_power_of_two(*n))
      }
      SizeClass::BoundedMax(n) => {
        std::cmp::min(STANDARD_BUFFER_SIZE, usize::next_power_of_two(*n))
      }
      SizeClass::Between(min, _max) => {
        std::cmp::max(STANDARD_BUFFER_SIZE, usize::next_power_of_two(*min))
      }
    };

    // Ensure we allocate at least TINY_BUFFER_SIZE
    std::cmp::max(size, TINY_BUFFER_SIZE)
  }

  /// The amount of data we expect to read from this stream (or `usize::MAX` if we just don't know).
  fn expected_remaining(&self) -> usize {
    match self {
      SizeClass::Unknown => usize::MAX,
      SizeClass::LikelyEmpty => 0,
      SizeClass::LikelyKnown(n) => *n,
      SizeClass::BoundedMin(_n) => usize::MAX,
      SizeClass::BoundedMax(n) => *n,
      SizeClass::Between(_min, max) => *max,
    }
  }
}

/// Assists code that reads from a hinted stream to determine the size of buffers.
///
/// `AdaptiveBufferStrategy` is designed to facilitate efficient reading from a stream
/// with dynamic buffer sizing. It adapts the buffer size based on the provided hints and read patterns.
pub struct AdaptiveBufferStrategy {
  /// The number of bytes we attempt to grow the buffer by each time it fills
  /// up and we have more data to read. We start at 64 KB. The grow_len is
  /// doubled if the nread returned from a single read is equal or greater than
  /// the grow_len. This allows us to reduce allocations for resources that can
  /// read large chunks of data at a time.
  grow_len: usize,
  expected_remaining: usize,
}

impl AdaptiveBufferStrategy {
  #[cfg(test)]
  fn new_from_size_class(size_class: SizeClass) -> Self {
    let hint = size_class.to_size_hint();
    Self::new_from_hint(hint.0, hint.1)
  }

  pub fn new_from_hint_u64(min: u64, maybe_max: Option<u64>) -> Self {
    Self::new_from_hint(min as _, maybe_max.map(|m| m as _))
  }

  pub fn new_from_hint(min: usize, maybe_max: Option<usize>) -> Self {
    let size_class = SizeClass::from_size_hint(min, maybe_max);

    // Try to determine our expected length and optimal starting buffer size for this resource based
    // on the size hint.

    let grow_len = size_class.initial_buffer_size();
    let expected_remaining = size_class.expected_remaining();

    Self {
      grow_len,
      expected_remaining,
    }
  }

  pub fn buffer_size(&self) -> usize {
    self.grow_len
  }

  pub fn notify_read(&mut self, nread: usize) {
    // If we managed to read more or equal data than fits in a single grow_len in
    // a single go, let's attempt to read even more next time. this reduces
    // allocations for resources that can read large chunks of data at a time.
    //
    // Note that we don't continue growing the buffer if we think we are close to the
    // end (expected_remaining is zero), or we've hit the MAX_GROW_LENGTH.
    self.expected_remaining = self.expected_remaining.saturating_sub(nread);
    if nread >= self.grow_len
      && self.grow_len < MAX_GROW_LEN
      && self.expected_remaining > 0
    {
      self.grow_len *= 2;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  trait ReadReporter {
    fn read(&mut self, n: usize) -> usize;
  }

  struct MaxReader {
    max_read_size: usize,
    total_size: usize,
  }

  impl ReadReporter for MaxReader {
    fn read(&mut self, n: usize) -> usize {
      let n =
        std::cmp::min(self.total_size, std::cmp::min(n, self.max_read_size));
      self.total_size = self.total_size.checked_sub(n).unwrap();
      n
    }
  }

  fn drain_reader(
    mut strategy: AdaptiveBufferStrategy,
    mut reader: impl ReadReporter,
  ) -> Vec<usize> {
    let mut res = vec![];
    loop {
      let size = strategy.buffer_size();
      let read = reader.read(size);
      if read == 0 {
        break;
      }
      res.push(read);
      strategy.notify_read(read);
    }
    res
  }

  #[test]
  fn resource_no_hint() {
    for (class, reader, expected) in [
      (
        SizeClass::Unknown,
        MaxReader {
          max_read_size: 8 * 1024,
          total_size: 10 * 1024,
        },
        vec![8192, 2048],
      ),
      (
        SizeClass::Unknown,
        MaxReader {
          max_read_size: 128 * 1024,
          total_size: 512 * 1024,
        },
        vec![65536, 131072, 131072, 131072, 65536],
      ),
      (
        SizeClass::Unknown,
        MaxReader {
          max_read_size: 1024 * 1024,
          total_size: 1024 * 1024,
        },
        vec![65536, 131072, 262144, 524288, 65536],
      ),
      (
        SizeClass::Unknown,
        MaxReader {
          max_read_size: usize::MAX,
          total_size: 64 * 1024 * 1024,
        },
        vec![
          65536, 131072, 262144, 524288, 1048576, 2097152, 4194304, 8388608,
          16777216, 16777216, 16777216, 65536,
        ],
      ),
    ] {
      let strategy = AdaptiveBufferStrategy::new_from_size_class(class);
      assert_eq!(drain_reader(strategy, reader), expected);
    }
  }

  #[test]
  fn resource_known_hint() {
    for (class, reader, expected) in [
      (
        SizeClass::LikelyKnown(10 * 1024),
        MaxReader {
          max_read_size: 8 * 1024,
          total_size: 10 * 1024,
        },
        vec![8192, 2048],
      ),
      // A resource that lies.
      (
        SizeClass::LikelyKnown(10 * 1024),
        MaxReader {
          max_read_size: 8 * 1024,
          total_size: 16 * 1024,
        },
        vec![8192, 8192],
      ),
    ] {
      let strategy = AdaptiveBufferStrategy::new_from_size_class(class);
      assert_eq!(drain_reader(strategy, reader), expected);
    }
  }
}
