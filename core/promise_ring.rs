use crate::PromiseId;
use std::collections::btree_map::BTreeMap;

type PromiseResolver = v8::Global<v8::PromiseResolver>;
const RING_SIZE: usize = 4 * 1024;

// NOTE: could generalize to a "RingMap"
pub struct PromiseRing {
  len: u32,
  cursor: u32,
  // TODO(@AaronO): Maybe optimize by tracking existence with a bitset
  // would reduce mem usage from 2*usize*RING_SIZE to (1+1/8)*usize*RING_SIZE
  // with the current approach the Option tag "wastes" 32kb per isolate
  ring: Vec<Option<PromiseResolver>>,
  map: BTreeMap<PromiseId, PromiseResolver>,
}

impl PromiseRing {
  pub(crate) fn new() -> Self {
    Self {
      len: 0,
      cursor: 0,
      ring: vec![None; RING_SIZE],
      map: BTreeMap::new(),
    }
  }

  // TODO(@AaronO): decide if this is useful (but it should match length of futures stream so ...)
  // pub(crate) fn len(&self) -> u32 {
  //   self.len
  // }

  pub(crate) fn has(&self, id: PromiseId) -> bool {
    let ring_start = if self.cursor < (RING_SIZE as u32) {
      0
    } else {
      self.cursor - RING_SIZE as u32
    };
    if id >= ring_start {
      self.ring[Self::ring_idx(id)].is_some()
    } else {
      self.map.contains_key(&id)
    }
  }

  pub(crate) fn take(&mut self, id: PromiseId) -> Option<PromiseResolver> {
    let ring_start = if self.cursor < (RING_SIZE as u32) {
      0
    } else {
      self.cursor - RING_SIZE as u32
    };
    let resolver = if id >= ring_start {
      self.ring.get_mut(Self::ring_idx(id)).unwrap().take()
    } else {
      self.map.remove(&id)
    };
    if resolver.is_some() {
      self.len -= 1;
    }
    resolver
  }

  pub(crate) fn head(&self) -> PromiseId {
    self.cursor as PromiseId
  }

  pub(crate) fn put(&mut self, resolver: PromiseResolver) -> PromiseId {
    let id = self.cursor;
    self.cursor += 1;
    self.len += 1;
    let slot = self.ring.get_mut(Self::ring_idx(id));
    if let Some(old_resolver) = slot.unwrap().replace(resolver) {
      let old_id = id - RING_SIZE as u32; // Since we're looping on the ring
      self.map.insert(old_id, old_resolver);
    }
    id as PromiseId
  }

  fn ring_idx(id: u32) -> usize {
    (id as usize) % RING_SIZE
  }
}
