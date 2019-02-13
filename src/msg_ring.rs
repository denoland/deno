use std::marker;
use std::mem::{forget, size_of};
use std::ops::{Add, BitAnd, Deref, DerefMut, Not, Sub};
use std::slice;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::futex::Futex;

#[derive(Clone, Copy, Debug, Default)]
pub struct Counters {
  pub message: usize,
  pub acquire: usize,
  pub release: usize,
  pub spin: usize,
  pub wait: usize,
  pub notify: usize,
  pub wrap: usize,
}

#[derive(Clone, Copy)]
pub enum FillDirection {
  TopDown,
  BottomUp,
}

trait MapOffset {
  fn map_offset(&self, offset: usize, length: usize, end: usize) -> usize;
}

impl MapOffset for FillDirection {
  fn map_offset(&self, offset: usize, length: usize, end: usize) -> usize {
    match self {
      FillDirection::TopDown => offset,
      FillDirection::BottomUp => end - length - offset,
    }
  }
}

struct FrameAllocation;
#[allow(non_upper_case_globals)]
impl FrameAllocation {
  pub const Alignment: usize = 8;
  pub const HeaderByteLength: usize = 8;
}

struct FrameHeader;
#[allow(non_upper_case_globals)]
impl FrameHeader {
  // Using i32 since that's what's used on the JS side.
  pub const None: i32 = 0x00_00_00_00;
  pub const ByteLengthMask: i32 = 0x00_ff_ff_ff;
  pub const EpochMask: i32 = 0x03_00_00_00;
  pub const EpochInitSender: i32 = 0x00_00_00_00;
  pub const EpochInitReceiver: i32 = 0x01_00_00_00;
  pub const EpochIncrementPass: i32 = 0x01_00_00_00;
  pub const EpochIncrementWrap: i32 = 0x02_00_00_00;
  pub const HasMessageFlag: i32 = 0x04_00_00_00;
  pub const HasWaitersFlag: i32 = 0x08_00_00_00;
}

trait Align<T> {
  fn align(self, to: T) -> Self;
  fn is_aligned(self, to: T) -> bool;
}

impl<T> Align<T> for T
where
  T: Copy
    + From<u8>
    + PartialEq
    + Add<Output = T>
    + Sub<Output = T>
    + BitAnd<Output = T>
    + Not<Output = T>,
{
  fn align(self, to: T) -> Self {
    let mask = to - 1.into();
    (self + mask) & !mask
  }

  fn is_aligned(self, to: T) -> bool {
    self & (to - 1.into()) == 0.into()
  }
}

enum Dealloc {
  Void,
  Vec {
    ptr: *mut u8,
    len: usize,
    cap: usize,
  },
}

unsafe impl marker::Send for Dealloc {}

impl Drop for Dealloc {
  #[allow(clippy::match_ref_pats)] // Clippy is wrong, `&mut` is necessary.
  fn drop(&mut self) {
    if let &mut Dealloc::Vec { ptr, len, cap } = self {
      unsafe { Vec::<u8>::from_raw_parts(ptr, len, cap) };
    }
  }
}

pub struct Buffer {
  ptr: *mut u8,
  byte_length: usize,
  dealloc: Arc<Dealloc>,
}

unsafe impl marker::Send for Buffer {}
unsafe impl marker::Sync for Buffer {}

impl Buffer {
  pub fn new(byte_length: usize) -> Self {
    assert!(byte_length > 0);
    assert!(byte_length.is_aligned(FrameAllocation::Alignment));

    let mut vec: Vec<u8> = Vec::new();
    vec.resize(byte_length, 0);
    let ptr = vec.as_mut_ptr();
    let dealloc = Dealloc::Vec {
      ptr,
      len: vec.len(),
      cap: vec.capacity(),
    };
    forget(vec);

    Self {
      ptr,
      byte_length,
      dealloc: Arc::new(dealloc),
    }
  }

  pub unsafe fn from_raw_parts(ptr: *mut u8, byte_length: usize) -> Self {
    assert!(byte_length.is_aligned(FrameAllocation::Alignment as usize));
    Self {
      ptr,
      byte_length,
      dealloc: Arc::new(Dealloc::Void),
    }
  }

  unsafe fn dup(&self) -> Self {
    Self {
      ptr: self.ptr,
      byte_length: self.byte_length,
      dealloc: self.dealloc.clone(),
    }
  }

  pub fn byte_length(&self) -> usize {
    self.byte_length
  }

  #[allow(dead_code)]
  unsafe fn get<T>(&self, byte_offset: usize) -> &T {
    self.slice(byte_offset, size_of::<T>()).get_unchecked(0)
  }

  unsafe fn get_mut<T>(&mut self, byte_offset: usize) -> &mut T {
    self
      .slice_mut(byte_offset, size_of::<T>())
      .get_unchecked_mut(0)
  }

  unsafe fn slice<T>(&self, byte_offset: usize, byte_length: usize) -> &[T] {
    let (offset, count) = self.map_bytes_to::<T>(byte_offset, byte_length);
    slice::from_raw_parts((self.ptr as *mut T).add(offset), count)
  }

  unsafe fn slice_mut<T>(
    &mut self,
    byte_offset: usize,
    byte_length: usize,
  ) -> &mut [T] {
    let (offset, count) = self.map_bytes_to::<T>(byte_offset, byte_length);
    slice::from_raw_parts_mut((self.ptr as *mut T).add(offset), count)
  }

  fn map_bytes_to<T>(
    &self,
    byte_offset: usize,
    byte_length: usize,
  ) -> (usize, usize) {
    let bytes_per_item = size_of::<T>();
    assert!(byte_offset + byte_length <= self.byte_length);
    debug_assert!(byte_offset.is_aligned(bytes_per_item));
    debug_assert!(byte_length.is_aligned(bytes_per_item));
    (byte_offset / bytes_per_item, byte_length / bytes_per_item)
  }
}

#[derive(Clone, Copy)]
pub struct Config {
  pub fill_direction: FillDirection,
  pub spin_count: u32,
  // TODO maybe:
  //pub spin_yield_cpu_time: u32,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      fill_direction: FillDirection::TopDown,
      spin_count: 1000,
      // spin_yield_cpu_time: 0,
    }
  }
}

// TODO: probably should use builder pattern.
pub struct MsgRing {
  buffer: Buffer,
  config: Config,
}

impl MsgRing {
  pub fn new(buffer: Buffer) -> Self {
    Self {
      buffer,
      config: Default::default(),
    }
  }

  pub fn new_with(buffer: Buffer, config: Config) -> Self {
    Self { buffer, config }
  }

  pub fn split(self) -> (Sender, Receiver) {
    let sender = Sender::new(unsafe { self.buffer.dup() }, self.config);
    let receiver = Receiver::new(self.buffer, self.config);
    (sender, receiver)
  }
}

struct Window {
  pub buffer: Buffer,
  pub config: Config,
  pub counters: Counters,

  // Head and tail position are in bytes, but always starting at zero and
  // not adjusted for buffer fill direction (see TypeScript implementation).
  pub epoch: i32,
  pub tail_position: usize,
  pub head_position: usize,
}

impl Window {
  pub fn new(buffer: Buffer, config: Config, epoch: i32) -> Self {
    let mut this = Self {
      buffer,
      config,
      counters: Counters {
        ..Default::default()
      },
      epoch,
      head_position: 0,
      tail_position: 0,
    };
    this.init();
    this
  }

  #[inline]
  pub fn byte_length(&self) -> usize {
    self.head_position - self.tail_position
  }

  #[inline]
  fn is_at_end_of_buffer(&self) -> bool {
    self.head_position == self.buffer.byte_length()
  }

  #[inline]
  fn header_byte_offset(&self, position: usize) -> usize {
    self.config.fill_direction.map_offset(
      position,
      FrameAllocation::HeaderByteLength,
      self.buffer.byte_length(),
    )
  }

  #[inline]
  fn get_message_byte_range(&self, frame_byte_length: usize) -> (usize, usize) {
    let message_byte_length =
      frame_byte_length - FrameAllocation::HeaderByteLength;
    let message_byte_offset = self.config.fill_direction.map_offset(
      self.tail_position + FrameAllocation::HeaderByteLength,
      message_byte_length,
      self.buffer.byte_length(),
    );
    (message_byte_offset, message_byte_length)
  }

  pub fn get_message_slice(&self, frame_byte_length: usize) -> &[u8] {
    let (byte_offset, byte_length) =
      self.get_message_byte_range(frame_byte_length);
    unsafe { self.buffer.slice(byte_offset, byte_length) }
  }

  pub fn get_message_slice_mut(
    &mut self,
    frame_byte_length: usize,
  ) -> &mut [u8] {
    let (byte_offset, byte_length) =
      self.get_message_byte_range(frame_byte_length);
    unsafe { self.buffer.slice_mut(byte_offset, byte_length) }
  }

  fn init(&mut self) {
    let target = self.buffer.byte_length() as i32;
    let header_byte_offset = self.header_byte_offset(0);
    let header_atomic: &mut Futex =
      unsafe { self.buffer.get_mut(header_byte_offset) };
    header_atomic.compare_and_swap(0, target, Ordering::AcqRel);
  }

  pub fn acquire_frame(&mut self, wait: bool) -> i32 {
    if self.is_at_end_of_buffer() {
      assert!(self.byte_length() == 0);

      self.epoch =
        (self.epoch + FrameHeader::EpochIncrementWrap) & FrameHeader::EpochMask;

      self.head_position = 0;
      self.tail_position = 0;
      self.counters.wrap += 1;
    }

    let header_byte_offset = self.header_byte_offset(self.head_position);
    let header_atomic: &mut Futex =
      unsafe { self.buffer.get_mut(header_byte_offset) };
    let mut header = *header_atomic.get_mut();

    let mut spin_count_remaining = self.config.spin_count;
    let mut sleep = false;

    // Note that operator precendece in Rust is different than in C and
    // JavaScript (& has higher precedence than ==), so this is correct.
    while header & FrameHeader::EpochMask != self.epoch {
      if !wait {
        return FrameHeader::None;
      }

      if spin_count_remaining == 0 {
        let expect = header;
        let target = header | FrameHeader::HasWaitersFlag;
        header =
          header_atomic.compare_and_swap(expect, target, Ordering::AcqRel);
        if expect != header {
          continue;
        }
        header = target;
        sleep = true;
        self.counters.wait += 1;
      } else {
        spin_count_remaining -= 1;
        self.counters.spin += 1;
      }

      if sleep {
        header_atomic.wait(header, None);
      }
      header = header_atomic.load(Ordering::Acquire);
    }

    let byte_length = header & FrameHeader::ByteLengthMask;
    let byte_length = byte_length as usize;
    assert!(byte_length <= self.buffer.byte_length() - self.head_position);

    self.head_position += byte_length;
    self.counters.acquire += 1;

    header
  }

  pub fn release_frame(&mut self, byte_length: usize, flags: i32) {
    assert!(byte_length >= FrameAllocation::HeaderByteLength);
    assert!(byte_length <= self.byte_length());

    let tail_epoch = self.epoch + FrameHeader::EpochIncrementPass;
    let new_header = byte_length as i32 | flags | tail_epoch;

    let header_byte_offset = self.header_byte_offset(self.tail_position);
    let header_atomic: &mut Futex =
      unsafe { self.buffer.get_mut(header_byte_offset) };

    let old_header = header_atomic.swap(new_header, Ordering::AcqRel);

    if old_header & FrameHeader::HasWaitersFlag != 0 {
      header_atomic.notify_one();
      self.counters.notify += 1;
    }

    self.tail_position += byte_length;
    self.counters.release += 1;
  }
}

pub struct Sender {
  window: Window,
}

impl Sender {
  pub fn new(buffer: Buffer, config: Config) -> Self {
    Self {
      window: Window::new(buffer, config, FrameHeader::EpochInitSender),
    }
  }

  pub fn compose(&mut self, byte_length: usize) -> Send {
    Send::new(&mut self.window, byte_length)
  }

  pub fn counters(&self) -> Counters {
    self.window.counters
  }
}

pub struct Send<'msg> {
  window: &'msg mut Window,
  allocation_byte_length: usize,
}

impl<'msg> Send<'msg> {
  fn new(window: &'msg mut Window, message_byte_length: usize) -> Self {
    let mut this = Self {
      window,
      allocation_byte_length: 0,
    };
    this.allocate(message_byte_length);
    this
  }

  pub fn resize(&mut self, message_byte_length: usize) {
    self.allocate(message_byte_length)
  }

  pub fn send(self) {
    self.window.counters.message += 1;
    self
      .window
      .release_frame(self.allocation_byte_length, FrameHeader::HasMessageFlag);
  }

  pub fn dispose(self) {}

  fn allocate(&mut self, byte_length: usize) {
    self.allocation_byte_length = FrameAllocation::HeaderByteLength as usize
      + byte_length.align(FrameAllocation::Alignment as usize);
    assert!(
      self.allocation_byte_length <= FrameHeader::ByteLengthMask as usize
    );
    while self.window.byte_length() < self.allocation_byte_length {
      if self.window.is_at_end_of_buffer() && self.window.byte_length() > 0 {
        self
          .window
          .release_frame(self.window.byte_length(), FrameHeader::None);
      }
      self.window.acquire_frame(true);
    }
  }
}

impl<'msg> Deref for Send<'msg> {
  type Target = [u8];

  fn deref(&self) -> &[u8] {
    self.window.get_message_slice(self.allocation_byte_length)
  }
}

impl<'msg> DerefMut for Send<'msg> {
  fn deref_mut(&mut self) -> &mut [u8] {
    self
      .window
      .get_message_slice_mut(self.allocation_byte_length)
  }
}

pub struct Receiver {
  window: Window,
}

// TODO: validate this.
// I suspect Receiver is Send but not Sync.
unsafe impl marker::Send for Receiver {}

impl Receiver {
  pub fn new(buffer: Buffer, config: Config) -> Self {
    Self {
      window: Window::new(buffer, config, FrameHeader::EpochInitReceiver),
    }
  }

  pub fn receive(&mut self) -> Receive {
    Receive::new(&mut self.window)
  }

  pub fn counters(&self) -> Counters {
    self.window.counters
  }
}

pub struct Receive<'msg: 'msg> {
  window: &'msg mut Window,
}

impl<'msg> Receive<'msg> {
  fn new(window: &'msg mut Window) -> Self {
    let mut this = Self { window };
    this.acquire();
    this
  }

  fn acquire(&mut self) {
    debug_assert_eq!(self.window.byte_length(), 0);
    while self.window.acquire_frame(true) & FrameHeader::HasMessageFlag == 0 {
      self
        .window
        .release_frame(self.window.byte_length(), FrameHeader::None);
    }
    self.window.counters.message += 1;
  }

  fn release(&mut self) {
    self
      .window
      .release_frame(self.window.byte_length(), FrameHeader::None);
  }

  pub fn dispose(self) {}
}

impl<'msg> Deref for Receive<'msg> {
  type Target = [u8];

  fn deref(&self) -> &[u8] {
    self.window.get_message_slice(self.window.byte_length())
  }
}

impl<'msg> Drop for Receive<'msg> {
  fn drop(&mut self) {
    self.release();
  }
}

#[cfg(test)]
mod test {
  use super::{Buffer, MsgRing};
  use std::sync::{Mutex, MutexGuard};
  use std::thread;
  use std::time::{Duration, Instant};

  const ROUNDS: usize = 10;
  const PER_ROUND: usize = 1e6 as usize;

  trait AsFloat {
    fn as_float(&self) -> f64;
  }
  impl AsFloat for Duration {
    #[allow(clippy::cast_lossless)]
    fn as_float(&self) -> f64 {
      self.as_secs() as f64 + self.subsec_nanos() as f64 / 1e9
    }
  }

  #[test] // TODO: use #[bench].
  fn uni_flow_benchmark() {
    let _guard = unparallelize_test();

    let buffer = Buffer::new(240);
    let ring = MsgRing::new(buffer);
    let (mut sender, mut receiver) = ring.split();

    let thread1 = thread::spawn(move || {
      benchmark_loop(
        "sender",
        &mut (&mut sender, 0),
        |(sender, ctr)| {
          let mut send = sender.compose(32);
          send[*ctr >> 8 & 7] = *ctr as u8;
          *ctr += 1;
          send.send();
        },
        |(sender, _)| eprintln!("  s {:?}", sender.counters()),
      );
    });

    let thread2 = thread::spawn(move || {
      benchmark_loop(
        "recver",
        &mut (&mut receiver, 0),
        |(receiver, ctr)| {
          let msg = receiver.receive();
          assert_eq!(msg[*ctr >> 8 & 7], *ctr as u8);
          *ctr += 1;
        },
        |(receiver, _)| eprintln!("  r {:?}", receiver.counters()),
      );
    });

    thread1.join().unwrap();
    thread2.join().unwrap();
  }

  #[test] // TODO: use #[bench]
  fn ping_pong_benchmark() {
    let _guard = unparallelize_test();

    let (mut sender1, mut receiver1) = MsgRing::new(Buffer::new(10240)).split();
    let (mut sender2, mut receiver2) = MsgRing::new(Buffer::new(10240)).split();

    let thread1 = thread::spawn(move || {
      benchmark_loop(
        "send..recv",
        &mut (&mut sender1, &mut receiver2),
        |(sender, receiver)| {
          sender.compose(32).send();
          receiver.receive();
        },
        |(sender, receiver)| {
          eprintln!("  s1 {:?}", sender.counters());
          eprintln!("  r2 {:?}", receiver.counters());
        },
      );
    });

    let thread2 = thread::spawn(move || {
      benchmark_loop(
        "recv..send",
        &mut (&mut sender2, &mut receiver1),
        |(sender, receiver)| {
          receiver.receive();
          sender.compose(32).send();
        },
        |(sender, receiver)| {
          eprintln!("  s2 {:?}", sender.counters());
          eprintln!("  r1 {:?}", receiver.counters());
        },
      );
    });

    thread1.join().unwrap();
    thread2.join().unwrap();
  }

  fn benchmark_loop<A, RoundFn: Fn(&mut A), StatsFn: Fn(&mut A)>(
    name: &str,
    a: &mut A,
    round_fn: RoundFn,
    stats_fn: StatsFn,
  ) {
    let tid = thread::current().id();

    for round in 0..ROUNDS {
      let start_time = Instant::now();

      for _ in 0..PER_ROUND {
        round_fn(a);
      }

      let elapsed_time: Duration = Instant::now() - start_time;
      let elapsed_time = elapsed_time.as_float();
      let rate = (PER_ROUND as f64 / elapsed_time) as u64;
      eprintln!(
        "round {}, {:?}, {}, count: {}, rate: {} s\u{207b}\u{b9}",
        round, tid, name, PER_ROUND, rate
      );
      stats_fn(a);
    }
  }

  fn unparallelize_test() -> MutexGuard<'static, ()> {
    lazy_static! {
      static ref GLOBAL_MUTEX: Mutex<()> = Mutex::new(());
    };
    GLOBAL_MUTEX.lock().unwrap()
  }
}
