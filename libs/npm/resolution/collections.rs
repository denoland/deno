// Copyright 2018-2026 the Deno authors. MIT license.

enum OneDirectionalLinkedListItem<'a, T> {
  Root,
  Item(&'a T),
}

/// A linked list that only points at the previously added item.
pub struct OneDirectionalLinkedList<'a, T> {
  parent: Option<&'a OneDirectionalLinkedList<'a, T>>,
  value: OneDirectionalLinkedListItem<'a, T>,
}

impl<T> Default for OneDirectionalLinkedList<'_, T> {
  fn default() -> Self {
    Self {
      parent: None,
      value: OneDirectionalLinkedListItem::Root,
    }
  }
}

impl<'a, T> OneDirectionalLinkedList<'a, T> {
  pub fn push(&'a self, item: &'a T) -> Self {
    Self {
      parent: Some(self),
      value: OneDirectionalLinkedListItem::Item(item),
    }
  }

  pub fn iter(&'a self) -> OneDirectionalLinkedListIterator<'a, T> {
    OneDirectionalLinkedListIterator { next: Some(self) }
  }
}

pub struct OneDirectionalLinkedListIterator<'a, T> {
  next: Option<&'a OneDirectionalLinkedList<'a, T>>,
}

impl<'a, T> Iterator for OneDirectionalLinkedListIterator<'a, T> {
  type Item = &'a T;

  fn next(&mut self) -> Option<Self::Item> {
    match self.next.take() {
      Some(ancestor_id_node) => {
        self.next = ancestor_id_node.parent;
        match ancestor_id_node.value {
          OneDirectionalLinkedListItem::Root => None,
          OneDirectionalLinkedListItem::Item(id) => Some(id),
        }
      }
      None => None,
    }
  }
}

#[cfg(test)]
mod test {
  use super::OneDirectionalLinkedList;

  #[test]
  fn test_linked_list() {
    let list = OneDirectionalLinkedList::default();
    let item1 = list.push(&1);
    let item2 = item1.push(&2);
    let item3 = item2.push(&3);
    assert_eq!(item3.iter().copied().collect::<Vec<_>>(), vec![3, 2, 1]);
    assert_eq!(item2.iter().copied().collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(item1.iter().copied().collect::<Vec<_>>(), vec![1]);
    assert_eq!(
      list.iter().copied().collect::<Vec<_>>(),
      Vec::<usize>::new()
    );
    assert_eq!(item3.iter().copied().collect::<Vec<_>>(), vec![3, 2, 1]);
  }
}
