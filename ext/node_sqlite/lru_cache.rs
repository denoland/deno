// Copyright 2018-2026 the Deno authors. MIT license.

// Ported from https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/src/lru_cache-inl.h

use std::collections::HashMap;
use std::collections::VecDeque;
use std::hash::Hash;

pub struct LRUCache<K, V>
where
  K: Eq + Hash + Clone,
{
  capacity: usize,
  order: VecDeque<K>,
  map: HashMap<K, V>,
}

impl<K, V> LRUCache<K, V>
where
  K: Eq + Hash + Clone,
{
  pub fn new(capacity: usize) -> Self {
    LRUCache {
      capacity,
      order: VecDeque::with_capacity(capacity),
      map: HashMap::with_capacity(capacity),
    }
  }

  pub fn put(&mut self, key: K, value: V) {
    if self.map.contains_key(&key) {
      self.order.retain(|k| k != &key);
      self.map.remove(&key);
    }

    self.order.push_front(key.clone());
    self.map.insert(key, value);

    if self.map.len() > self.capacity
      && let Some(lru_key) = self.order.pop_back()
    {
      self.map.remove(&lru_key);
    }
  }

  pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
    if self.map.contains_key(key) {
      self.order.retain(|k| k != key);
      self.order.push_front(key.clone());
      self.map.get_mut(key)
    } else {
      None
    }
  }

  pub fn erase(&mut self, key: &K) {
    if self.map.remove(key).is_some() {
      self.order.retain(|k| k != key);
    }
  }

  pub fn exists(&self, key: &K) -> bool {
    self.map.contains_key(key)
  }

  pub fn size(&self) -> usize {
    self.map.len()
  }

  pub fn clear(&mut self) {
    self.order.clear();
    self.map.clear();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Test basic Put and Get operations
  #[test]
  fn put_and_get() {
    let mut cache = LRUCache::new(2);
    cache.put(1, "one");
    cache.put(2, "two");

    assert!(cache.exists(&1));
    assert_eq!(cache.get_mut(&1), Some(&mut "one"));

    assert!(cache.exists(&2));
    assert_eq!(cache.get_mut(&2), Some(&mut "two"));

    assert!(!cache.exists(&3));
  }

  // Test that putting an existing key updates its value and moves it to the front
  #[test]
  fn put_updates_existing() {
    let mut cache = LRUCache::new(2);
    cache.put(1, "one");
    cache.put(2, "two");
    cache.put(1, "updated one");

    assert_eq!(cache.size(), 2);
    assert_eq!(cache.get_mut(&1), Some(&mut "updated one"));

    // Now, if we add another element, key 2 should be evicted, not key 1
    cache.put(3, "three");
    assert!(!cache.exists(&2));
    assert!(cache.exists(&1));
    assert!(cache.exists(&3));
  }

  // Test the eviction of the least recently used item
  #[test]
  fn eviction() {
    let mut cache = LRUCache::new(3);
    cache.put(1, 10);
    cache.put(2, 20);
    cache.put(3, 30);

    // At this point, the order of use is 3, 2, 1
    cache.put(4, 40); // This should evict key 1

    assert_eq!(cache.size(), 3);
    assert!(!cache.exists(&1));
    assert!(cache.exists(&2));
    assert!(cache.exists(&3));
    assert!(cache.exists(&4));
  }

  // Test that get_mut() moves an item to the front (most recently used)
  #[test]
  fn get_moves_to_front() {
    let mut cache = LRUCache::new(2);
    cache.put('a', 1);
    cache.put('b', 2);

    // Access 'a', making it the most recently used
    let _ = cache.get_mut(&'a');

    // Add 'c', which should evict 'b'
    cache.put('c', 3);

    assert_eq!(cache.size(), 2);
    assert!(cache.exists(&'a'));
    assert!(!cache.exists(&'b'));
    assert!(cache.exists(&'c'));
  }

  // Test the erase() method
  #[test]
  fn erase() {
    let mut cache = LRUCache::new(2);
    cache.put(1, "one");
    cache.put(2, "two");

    cache.erase(&1);

    assert_eq!(cache.size(), 1);
    assert!(!cache.exists(&1));
    assert!(cache.exists(&2));

    // Erasing a non-existent key should not fail
    cache.erase(&99);
    assert_eq!(cache.size(), 1);
  }

  // Test the exists() method
  #[test]
  fn exists() {
    let mut cache = LRUCache::new(1);
    cache.put(1, 100);

    assert!(cache.exists(&1));
    assert!(!cache.exists(&2));
  }

  // Test the size() method
  #[test]
  fn size() {
    let mut cache = LRUCache::new(5);
    assert_eq!(cache.size(), 0);

    cache.put(1, 1);
    assert_eq!(cache.size(), 1);
    cache.put(2, 2);
    assert_eq!(cache.size(), 2);

    cache.put(1, 11); // Update
    assert_eq!(cache.size(), 2);

    cache.erase(&2);
    assert_eq!(cache.size(), 1);
  }

  // Test with a capacity of 0
  #[test]
  fn zero_size_cache() {
    let mut cache = LRUCache::new(0);
    cache.put(1, 1);
    assert!(!cache.exists(&1));
    assert_eq!(cache.size(), 0);
  }

  // Test with a capacity of 1
  #[test]
  fn one_size_cache() {
    let mut cache = LRUCache::new(1);
    cache.put(1, 1);
    assert!(cache.exists(&1));
    assert_eq!(cache.size(), 1);

    cache.put(2, 2);
    assert!(!cache.exists(&1));
    assert!(cache.exists(&2));
    assert_eq!(cache.size(), 1);
  }

  // Test with complex key and value types
  #[test]
  fn complex_types() {
    let mut cache: LRUCache<String, Vec<i32>> = LRUCache::new(2);
    let vec1 = vec![1, 2, 3];
    let vec2 = vec![4, 5, 6];
    let vec3 = vec![7, 8, 9];

    cache.put("vec1".to_string(), vec1.clone());
    cache.put("vec2".to_string(), vec2.clone());

    assert_eq!(cache.get_mut(&"vec1".to_string()), Some(&mut vec1.clone()));
    assert_eq!(cache.get_mut(&"vec2".to_string()), Some(&mut vec2.clone()));

    cache.put("vec3".to_string(), vec3);
    assert!(!cache.exists(&"vec1".to_string()));
    assert!(cache.exists(&"vec2".to_string()));
    assert!(cache.exists(&"vec3".to_string()));
  }

  // Test the clear() method
  #[test]
  fn clear() {
    let mut cache = LRUCache::new(3);
    cache.put(1, "one");
    cache.put(2, "two");
    cache.put(3, "three");

    assert_eq!(cache.size(), 3);

    cache.clear();

    assert_eq!(cache.size(), 0);
    assert!(!cache.exists(&1));
    assert!(!cache.exists(&2));
    assert!(!cache.exists(&3));
  }
}
