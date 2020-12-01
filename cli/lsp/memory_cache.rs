// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::fmt;
use std::mem;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileId(pub u32);

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ChangeKind {
  Create,
  Modify,
  Delete,
}

pub struct ChangedFile {
  pub change_kind: ChangeKind,
  pub file_id: FileId,
}

#[derive(Default)]
struct SpecifierInterner {
  map: HashMap<ModuleSpecifier, FileId>,
  vec: Vec<ModuleSpecifier>,
}

impl SpecifierInterner {
  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<FileId> {
    self.map.get(specifier).copied()
  }

  pub fn intern(&mut self, specifier: ModuleSpecifier) -> FileId {
    if let Some(id) = self.get(&specifier) {
      return id;
    }
    let id = FileId(self.vec.len() as u32);
    self.map.insert(specifier.clone(), id);
    self.vec.push(specifier);
    id
  }

  pub fn lookup(&self, id: FileId) -> &ModuleSpecifier {
    &self.vec[id.0 as usize]
  }
}

#[derive(Default)]
pub struct MemoryCache {
  data: Vec<Option<Vec<u8>>>,
  interner: SpecifierInterner,
  changes: Vec<ChangedFile>,
}

impl MemoryCache {
  fn alloc_file_id(&mut self, specifier: ModuleSpecifier) -> FileId {
    let file_id = self.interner.intern(specifier);
    let idx = file_id.0 as usize;
    let len = self.data.len().max(idx + 1);
    self.data.resize_with(len, || None);
    file_id
  }

  fn get(&self, file_id: FileId) -> &Option<Vec<u8>> {
    &self.data[file_id.0 as usize]
  }

  pub fn get_contents(&self, file_id: FileId) -> Result<String, AnyError> {
    String::from_utf8(self.get(file_id).as_deref().unwrap().to_vec())
      .map_err(|err| err.into())
  }

  fn get_mut(&mut self, file_id: FileId) -> &mut Option<Vec<u8>> {
    &mut self.data[file_id.0 as usize]
  }

  pub fn get_specifier(&self, file_id: FileId) -> &ModuleSpecifier {
    self.interner.lookup(file_id)
  }

  pub fn len(&self) -> usize {
    self.data.len()
  }

  pub fn lookup(&self, specifier: &ModuleSpecifier) -> Option<FileId> {
    self
      .interner
      .get(specifier)
      .filter(|&it| self.get(it).is_some())
  }

  pub fn set_contents(
    &mut self,
    specifier: ModuleSpecifier,
    contents: Option<Vec<u8>>,
  ) {
    let file_id = self.alloc_file_id(specifier);
    let change_kind = match (self.get(file_id), &contents) {
      (None, None) => return,
      (None, Some(_)) => ChangeKind::Create,
      (Some(_), None) => ChangeKind::Delete,
      (Some(old), Some(new)) if old == new => return,
      (Some(_), Some(_)) => ChangeKind::Modify,
    };

    *self.get_mut(file_id) = contents;
    self.changes.push(ChangedFile {
      file_id,
      change_kind,
    })
  }

  pub fn take_changes(&mut self) -> Vec<ChangedFile> {
    mem::take(&mut self.changes)
  }
}

impl fmt::Debug for MemoryCache {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("MemoryCache")
      .field("no_files", &self.data.len())
      .finish()
  }
}
