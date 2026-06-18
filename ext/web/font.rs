// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use cosmic_text::fontdb;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use deno_permissions::PermissionsContainer;
use serde::Serialize;

use crate::css::font::parse_css_font;
use crate::css::font::parse_css_stretch;
use crate::css::font::parse_css_style;
use crate::css::font::parse_css_weight;
use crate::css::font::stretch_to_css_str;
use crate::css::font::style_to_css_str;

/// System font metadata shared across all workers.
/// Populated once by `Deno.loadSystemFonts()`, then each worker copies
/// the FaceInfo entries into its own FontSystem via `push_face_info`.
#[derive(Clone, Default)]
pub struct SharedSystemFontDb(Arc<Mutex<SharedSystemFontDbInner>>);

#[derive(Default)]
struct SharedSystemFontDbInner {
  faces: Vec<fontdb::FaceInfo>,
  loaded: bool,
}

/// Maps u32 handles to font data.
/// bytes_store holds validated bytes; active_faces tracks what is in fontdb.
#[derive(Default)]
pub struct FontRegistry {
  next_handle: u32,
  bytes_store: HashMap<u32, Vec<u8>>,
  active_faces: HashMap<u32, Vec<fontdb::ID>>,
}

/// Metadata extracted from a font file.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FontLoadResult {
  handle: u32,
  weight: u16,
  style: &'static str,
  stretch: &'static str,
  unicode_coverage: Vec<[u32; 2]>,
}

/// Returns a sorted list of [start, end] codepoint ranges covered by the font.
fn extract_unicode_coverage(bytes: &[u8]) -> Vec<[u32; 2]> {
  let Ok(face) = ttf_parser::Face::parse(bytes, 0) else {
    return vec![];
  };
  let Some(cmap) = face.tables().cmap else {
    return vec![];
  };
  let mut codepoints: Vec<u32> = Vec::new();
  for subtable in cmap.subtables {
    if subtable.is_unicode() {
      subtable.codepoints(|cp| codepoints.push(cp));
      break; // use the first unicode subtable
    }
  }
  codepoints.sort_unstable();
  codepoints.dedup();
  let mut ranges: Vec<[u32; 2]> = Vec::new();
  let mut iter = codepoints.into_iter();
  let Some(first) = iter.next() else {
    return ranges;
  };
  let mut start = first;
  let mut end = first;
  for cp in iter {
    if cp == end + 1 {
      end = cp;
    } else {
      ranges.push([start, end]);
      start = cp;
      end = cp;
    }
  }
  ranges.push([start, end]);
  ranges
}

/// Async op: validates font bytes, extracts metadata, and stores them in bytes_store.
/// Does NOT add to the active fontdb.  Returns a FontLoadResult with an opaque handle.
#[op2]
#[serde]
pub async fn op_fontdb_load(
  state: Rc<RefCell<OpState>>,
  #[buffer] bytes: JsBuffer,
) -> Result<FontLoadResult, JsErrorBox> {
  let registry = state.borrow().borrow::<Arc<Mutex<FontRegistry>>>().clone();
  let bytes_vec = bytes.to_vec();

  // Validate and extract metadata in a blocking thread (TTF/OTF parsing can be slow).
  let (weight, style, stretch, unicode_coverage) =
    tokio::task::spawn_blocking({
      let bytes_for_validation = bytes_vec.clone();
      move || {
        let mut tmp = fontdb::Database::new();
        tmp.load_font_data(bytes_for_validation.clone());
        let Some(face_info) = tmp.faces().next() else {
          return Err(JsErrorBox::generic("No valid font faces in data"));
        };

        let weight = face_info.weight.0;
        let style = style_to_css_str(face_info.style);
        let stretch = stretch_to_css_str(face_info.stretch);

        let unicode_coverage = extract_unicode_coverage(&bytes_for_validation);

        Ok((weight, style, stretch, unicode_coverage))
      }
    })
    .await
    .map_err(|e| JsErrorBox::generic(e.to_string()))??;

  let mut reg = registry.lock().unwrap();
  let handle = reg.next_handle;
  reg.next_handle += 1;
  reg.bytes_store.insert(handle, bytes_vec);

  Ok(FontLoadResult {
    handle,
    weight,
    style,
    stretch,
    unicode_coverage,
  })
}

/// Sync op: loads stored bytes into the active fontdb (makes font available to canvas).
/// Descriptor parameters override the font binary's embedded metadata.
/// Pass empty strings to use the font's own metadata unchanged.
/// Idempotent: if already active, this is a no-op.
#[op2(fast)]
pub fn op_fontdb_add(
  state: &OpState,
  #[smi] handle: u32,
  #[string] family: &str,
  #[string] style: &str,
  #[string] weight: &str,
  #[string] stretch: &str,
) {
  let font_system = state
    .borrow::<Arc<Mutex<cosmic_text::FontSystem>>>()
    .clone();
  let registry = state.borrow::<Arc<Mutex<FontRegistry>>>().clone();
  let mut reg = registry.lock().unwrap();
  if reg.active_faces.contains_key(&handle) {
    return; // Idempotent.
  }
  let Some(bytes) = reg.bytes_store.get(&handle).cloned() else {
    return;
  };

  // Extract original FaceInfo metadata via a temporary database.
  let mut tmp_db = fontdb::Database::new();
  tmp_db.load_font_data(bytes.clone());

  let source: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
  let mut new_ids = Vec::new();

  for orig in tmp_db.faces().cloned().collect::<Vec<_>>() {
    let families = if family.is_empty() {
      orig.families.clone()
    } else {
      vec![(family.to_string(), fontdb::Language::English_UnitedStates)]
    };
    let info = fontdb::FaceInfo {
      id: orig.id, // overwritten by push_face_info
      source: fontdb::Source::Binary(Arc::clone(&source)),
      index: orig.index,
      families,
      post_script_name: orig.post_script_name.clone(),
      style: parse_css_style(style).unwrap_or(orig.style),
      weight: parse_css_weight(weight)
        .map(fontdb::Weight)
        .unwrap_or(orig.weight),
      stretch: parse_css_stretch(stretch).unwrap_or(orig.stretch),
      monospaced: orig.monospaced,
    };
    let id = font_system.lock().unwrap().db_mut().push_face_info(info);
    new_ids.push(id);
  }

  reg.active_faces.insert(handle, new_ids);
}

/// Sync op: removes font from the active fontdb.
/// bytes_store entry is kept so the handle can be re-activated via op_fontdb_add.
#[op2(fast)]
pub fn op_fontdb_remove(state: &OpState, #[smi] handle: u32) {
  let font_system = state
    .borrow::<Arc<Mutex<cosmic_text::FontSystem>>>()
    .clone();
  let registry = state.borrow::<Arc<Mutex<FontRegistry>>>().clone();
  let mut reg = registry.lock().unwrap();
  if let Some(ids) = reg.active_faces.remove(&handle) {
    let mut fs = font_system.lock().unwrap();
    for id in ids {
      fs.db_mut().remove_face(id);
    }
  }
  // bytes_store is NOT cleared; handle remains valid for re-add.
}

/// Sync op: fully unloads a font handle — removes active faces AND stored
/// bytes.  Called by the JS FinalizationRegistry when a FontFace is GC'd.
#[op2(fast)]
pub fn op_fontdb_unload(state: &OpState, #[smi] handle: u32) {
  let font_system = state
    .borrow::<Arc<Mutex<cosmic_text::FontSystem>>>()
    .clone();
  let registry = state.borrow::<Arc<Mutex<FontRegistry>>>().clone();
  let mut reg = registry.lock().unwrap();
  if let Some(ids) = reg.active_faces.remove(&handle) {
    let mut fs = font_system.lock().unwrap();
    for id in ids {
      fs.db_mut().remove_face(id);
    }
  }
  reg.bytes_store.remove(&handle);
}

#[derive(Serialize)]
struct CssFontQueryResult {
  family: String,
  style: String,
  weight: u16,
  stretch: String,
}

/// Sync op: parses a CSS font shorthand for FontFaceSet.check() / load() matching.
/// Returns null when the font string is syntactically invalid, uses a forbidden
/// keyword, or contains multiple comma-separated families.
#[op2]
#[serde]
pub fn op_parse_css_font_query(
  #[string] font: &str,
) -> Option<CssFontQueryResult> {
  let state = parse_css_font(font)?;
  // CSS Font Loading spec: a query font string must name exactly one family.
  if state.families.len() != 1 {
    return None;
  }
  let family = state.families.into_iter().next().unwrap();
  Some(CssFontQueryResult {
    family,
    style: style_to_css_str(state.style).to_string(),
    weight: state.weight,
    stretch: stretch_to_css_str(state.stretch).to_string(),
  })
}

#[op2(stack_trace)]
pub async fn op_fontdb_load_system_fonts(
  state: Rc<RefCell<OpState>>,
) -> Result<(), deno_permissions::PermissionCheckError> {
  let (shared_db, font_system) = {
    let st = state.borrow();
    let shared_db = st.borrow::<SharedSystemFontDb>().clone();
    let font_system =
      st.borrow::<Arc<Mutex<cosmic_text::FontSystem>>>().clone();
    (shared_db, font_system)
  };

  state
    .borrow_mut()
    .borrow_mut::<PermissionsContainer>()
    .check_sys("systemFonts", "Deno.loadSystemFonts")?;

  let faces = {
    let already_loaded = {
      let inner = shared_db.0.lock().unwrap();
      if inner.loaded {
        Some(inner.faces.clone())
      } else {
        None
      }
    };
    if let Some(faces) = already_loaded {
      faces
    } else {
      let discovered = tokio::task::spawn_blocking(|| {
        let mut tmp = fontdb::Database::new();
        tmp.load_system_fonts();
        tmp.faces().cloned().collect::<Vec<_>>()
      })
      .await
      .unwrap_or_default();
      let mut inner = shared_db.0.lock().unwrap();
      if !inner.loaded {
        inner.faces = discovered;
        inner.loaded = true;
      }
      inner.faces.clone()
    }
  };

  let mut fs = font_system.lock().unwrap();
  for face in faces {
    fs.db_mut().push_face_info(face);
  }

  Ok(())
}
