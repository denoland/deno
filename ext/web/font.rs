// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_permissions::PermissionsContainer;
use fontique::Blob;
use fontique::Collection;
use fontique::CollectionOptions;
use fontique::FontInfoOverride;
use fontique::FontStyle;
use fontique::FontWeight;
use fontique::FontWidth;
use fontique::SourceCache;
use parley::FontContext;
use serde::Deserialize;
use serde::Serialize;

use crate::blob::BlobStoreTrait;
use crate::css::font::CssFontStyle;
use crate::css::font::CssFontWidth;
use crate::css::font::FontSrc;
use crate::css::font::default_font_resolution;
use crate::css::font::is_generic_family;
use crate::css::font::normalize_font_face_family;
use crate::css::font::parse_css_font;
use crate::css::font::parse_css_font_src;
use crate::css::font::parse_css_style;
use crate::css::font::parse_css_weight;
use crate::css::font::parse_css_width;
use crate::css::font::style_to_css_str;
use crate::css::font::width_to_css_str;

/// Probe family for [`op_fontdb_load`] (empty names are skipped by register).
const PROBE_FAMILY_NAME: &str = "deno-font-face-probe";

/// Chunk size for [`op_fontdb_load_resource`], as in `ext/http`.
const FONT_READ_CHUNK: usize = 64 * 1024;
/// Upper bound on how much [`op_fontdb_load_resource`] preallocates from an
/// untrusted Content-Length.
const MAX_FONT_PREALLOC: u64 = 32 * 1024 * 1024;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FontError {
  #[class("DOMExceptionSyntaxError")]
  #[error("No valid font faces in data")]
  NoValidFaces,
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Read(#[from] JsErrorBox),
  #[class(generic)]
  #[error("{0}")]
  Join(String),
}

/// FontFace descriptor overrides for fontique (`None` = keep file value).
#[derive(Clone, Debug, Default)]
struct FaceOverride {
  family: Option<String>,
  style: Option<FontStyle>,
  weight: Option<FontWeight>,
  width: Option<FontWidth>,
}

impl FaceOverride {
  fn to_font_info_override(&self) -> FontInfoOverride<'_> {
    FontInfoOverride {
      family_name: self.family.as_deref(),
      width: self.width,
      style: self.style,
      weight: self.weight,
      axes: None,
    }
  }
}

/// A FontFace currently registered in the collection.
struct ActiveFace {
  handle: u32,
  overrides: FaceOverride,
}

/// Per-isolate FontFace data. Collection is rebuilt from `active`.
/// TODO(petamoriken): custom parley collection once linebender/parley#622.
#[derive(Default)]
pub struct FontRegistry {
  next_handle: u32,
  blobs: HashMap<u32, Blob<u8>>,
  active: Vec<ActiveFace>,
}

impl FontRegistry {
  fn insert_blob(&mut self, blob: Blob<u8>) -> u32 {
    let handle = self.next_handle;
    self.next_handle = handle.wrapping_add(1);
    self.blobs.insert(handle, blob);
    handle
  }

  /// Idempotent if already registered.
  fn register(
    &mut self,
    collection: &mut Collection,
    handle: u32,
    overrides: FaceOverride,
  ) {
    if self.active.iter().any(|face| face.handle == handle) {
      return;
    }
    // Unknown or already unloaded.
    let Some(blob) = self.blobs.get(&handle).cloned() else {
      return;
    };
    collection.register_fonts(blob, Some(overrides.to_font_info_override()));
    self.active.push(ActiveFace { handle, overrides });
  }

  /// Idempotent if unknown/inactive.
  fn unregister(&mut self, collection: &mut Collection, handle: u32) {
    let Some(pos) = self.active.iter().position(|face| face.handle == handle)
    else {
      return;
    };
    self.active.remove(pos);
    self.rebuild(collection);
  }

  /// Clear and re-register active faces. Avoids `unregister_font` (empty
  /// family shadowing + triple-key drops). Assumes sole writer of collection.
  fn rebuild(&self, collection: &mut Collection) {
    collection.clear();
    for face in &self.active {
      let Some(blob) = self.blobs.get(&face.handle).cloned() else {
        continue;
      };
      collection
        .register_fonts(blob, Some(face.overrides.to_font_info_override()));
    }
  }

  fn unload(&mut self, collection: &mut Collection, handle: u32) {
    self.unregister(collection, handle);
    self.blobs.remove(&handle);
  }
}

/// Per-isolate parley context (no system fonts until registerLocalFonts).
pub fn create_font_context() -> FontContext {
  FontContext {
    collection: Collection::new(CollectionOptions {
      shared: false,
      system_fonts: false,
    }),
    source_cache: SourceCache::default(),
  }
}

/// Borrow registry + collection together (not held across await/JS).
fn with_registry<R>(
  state: &OpState,
  f: impl FnOnce(&mut FontRegistry, &mut Collection) -> R,
) -> R {
  let registry = state.borrow::<Rc<RefCell<FontRegistry>>>();
  let font_ctx = state.borrow::<Rc<RefCell<FontContext>>>();
  let mut registry = registry.borrow_mut();
  let mut font_ctx = font_ctx.borrow_mut();
  f(&mut registry, &mut font_ctx.collection)
}

/// Metadata extracted from a font file.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FontLoadResult {
  handle: u32,
  weight: u16,
  style: &'static str,
  width: &'static str,
  unicode_coverage: Vec<[u32; 2]>,
}

/// Face-0 descriptors used when the caller did not set them.
struct FileMetadata {
  weight: u16,
  style: &'static str,
  width: &'static str,
  unicode_coverage: Vec<[u32; 2]>,
}

fn read_file_metadata(bytes: &[u8]) -> FileMetadata {
  let Ok(face) = ttf_parser::Face::parse(bytes, 0) else {
    // fontique may accept data ttf-parser cannot; use CSS defaults.
    return FileMetadata {
      weight: 400,
      style: "normal",
      width: "normal",
      unicode_coverage: vec![],
    };
  };
  // A Normal style with a non-zero italic angle counts as italic.
  let style = match face.style() {
    ttf_parser::Style::Italic => CssFontStyle::Italic,
    ttf_parser::Style::Oblique => CssFontStyle::Oblique,
    ttf_parser::Style::Normal if face.italic_angle() != 0.0 => {
      CssFontStyle::Italic
    }
    ttf_parser::Style::Normal => CssFontStyle::Normal,
  };
  let width = match face.width() {
    ttf_parser::Width::UltraCondensed => CssFontWidth::UltraCondensed,
    ttf_parser::Width::ExtraCondensed => CssFontWidth::ExtraCondensed,
    ttf_parser::Width::Condensed => CssFontWidth::Condensed,
    ttf_parser::Width::SemiCondensed => CssFontWidth::SemiCondensed,
    ttf_parser::Width::Normal => CssFontWidth::Normal,
    ttf_parser::Width::SemiExpanded => CssFontWidth::SemiExpanded,
    ttf_parser::Width::Expanded => CssFontWidth::Expanded,
    ttf_parser::Width::ExtraExpanded => CssFontWidth::ExtraExpanded,
    ttf_parser::Width::UltraExpanded => CssFontWidth::UltraExpanded,
  };
  FileMetadata {
    weight: face.weight().to_number(),
    style: style_to_css_str(style),
    width: width_to_css_str(width),
    unicode_coverage: extract_unicode_coverage(&face),
  }
}

/// Sorted `[start, end]` cmap ranges (check/load only).
/// TODO(petamoriken): render-time selection needs linebender/parley#622.
fn extract_unicode_coverage(face: &ttf_parser::Face) -> Vec<[u32; 2]> {
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

/// Validate + store bytes (not registered; see [`op_fontdb_add`]).
async fn load_font_data(
  state: &Rc<RefCell<OpState>>,
  bytes: Vec<u8>,
) -> Result<FontLoadResult, FontError> {
  // Parsing/cmap is slow for large CJK fonts.
  let (blob, meta) = tokio::task::spawn_blocking(move || {
    let meta = read_file_metadata(&bytes);
    let blob = Blob::new(Arc::new(bytes));
    // Probe via fontique so load success implies op_fontdb_add can register.
    let mut probe = Collection::new(CollectionOptions {
      shared: false,
      system_fonts: false,
    });
    let registered = probe.register_fonts(
      blob.clone(),
      Some(FontInfoOverride {
        family_name: Some(PROBE_FAMILY_NAME),
        ..Default::default()
      }),
    );
    if registered.is_empty() {
      return Err(FontError::NoValidFaces);
    }
    Ok((blob, meta))
  })
  .await
  .map_err(|e| FontError::Join(e.to_string()))??;

  let handle = {
    let state = state.borrow();
    let registry = state.borrow::<Rc<RefCell<FontRegistry>>>();
    registry.borrow_mut().insert_blob(blob)
  };

  Ok(FontLoadResult {
    handle,
    weight: meta.weight,
    style: meta.style,
    width: meta.width,
    unicode_coverage: meta.unicode_coverage,
  })
}

/// Load a BufferSource font. Detaches so V8 can drop the store now.
#[op2]
#[serde]
pub async fn op_fontdb_load(
  state: Rc<RefCell<OpState>>,
  #[buffer(detach)] bytes: JsBuffer,
) -> Result<FontLoadResult, FontError> {
  load_font_data(&state, bytes.to_vec()).await
}

/// Load a font from a fetch body rid. Bytes stay off the JS heap.
/// The caller keeps and closes the resource.
#[op2]
#[serde]
pub async fn op_fontdb_load_resource(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<FontLoadResult, FontError> {
  // `deno_web` cannot name the fetch response type, so go through the trait.
  let resource = state.borrow().resource_table.get_any(rid)?;
  // Content-Length is attacker-controlled, so treat it as a capped hint.
  let capacity = resource.size_hint().1.unwrap_or(0).min(MAX_FONT_PREALLOC);
  let mut bytes = Vec::with_capacity(capacity as usize);
  loop {
    // `read` (not `read_byob`): the fetch body returns a zero-copy view of
    // hyper's buffer, while the default `read_byob` adds a memcpy per chunk.
    let view = resource.clone().read(FONT_READ_CHUNK).await?;
    if view.is_empty() {
      break;
    }
    bytes.extend_from_slice(&view);
  }
  load_font_data(&state, bytes).await
}

/// Load font from blob store (null => fall through to next `src`).
#[op2]
#[serde]
pub async fn op_fontdb_load_object_url(
  state: Rc<RefCell<OpState>>,
  #[string] url: String,
) -> Result<Option<FontLoadResult>, FontError> {
  let blob = {
    let Ok(url) = Url::parse(&url) else {
      return Ok(None);
    };
    let state = state.borrow();
    let Some(blob_store) = state.try_borrow::<Arc<dyn BlobStoreTrait>>() else {
      return Ok(None);
    };
    let Some(blob) = blob_store.get_object_url(url) else {
      return Ok(None);
    };
    blob
  };

  let bytes = blob.read_all().await;
  load_font_data(&state, bytes).await.map(Some)
}

/// Match `local()` by PostScript name, family, then full name (name table).
fn find_local_font(db: &fontdb::Database, name: &str) -> Option<Vec<u8>> {
  for face in db.faces() {
    if face.post_script_name.eq_ignore_ascii_case(name)
      || face
        .families
        .iter()
        .any(|(family, _)| family.eq_ignore_ascii_case(name))
    {
      return db.with_face_data(face.id, |data, _| data.to_vec());
    }
  }
  for face in db.faces() {
    let matches = db
      .with_face_data(face.id, |data, index| {
        ttf_parser::Face::parse(data, index)
          .ok()
          .and_then(|face| extract_name(&face, ttf_parser::name_id::FULL_NAME))
          .is_some_and(|full_name| full_name.eq_ignore_ascii_case(name))
      })
      .unwrap_or(false);
    if matches {
      return db.with_face_data(face.id, |data, _| data.to_vec());
    }
  }
  None
}

/// Load a `local()` source (null => fall through to next `src`).
#[op2(stack_trace)]
#[serde]
pub async fn op_fontdb_load_local(
  state: Rc<RefCell<OpState>>,
  #[string] name: String,
) -> Result<Option<FontLoadResult>, FontError> {
  let shared_db = {
    let st = state.borrow();
    st.borrow::<SharedLocalFontDb>().clone()
  };

  state
    .borrow_mut()
    .borrow_mut::<PermissionsContainer>()
    .check_sys("localFonts", "new FontFace()")?;

  ensure_local_fonts_loaded(&shared_db).await;

  let bytes = tokio::task::spawn_blocking(move || {
    let inner = shared_db.inner.lock().unwrap();
    find_local_font(inner.db.as_ref()?, &name)
  })
  .await
  .map_err(|e| FontError::Join(e.to_string()))?;

  match bytes {
    Some(bytes) => load_font_data(&state, bytes).await.map(Some),
    None => Ok(None),
  }
}

/// Register stored bytes; empty descriptor strings keep file metadata.
#[op2(fast)]
pub fn op_fontdb_add(
  state: &OpState,
  #[smi] handle: u32,
  #[string] family: &str,
  #[string] style: &str,
  #[string] weight: &str,
  #[string] width: &str,
) {
  let overrides = FaceOverride {
    family: (!family.is_empty()).then(|| family.to_string()),
    style: parse_css_style(style).map(CssFontStyle::to_parley),
    weight: parse_css_weight(weight).map(|w| FontWeight::new(f32::from(w))),
    width: parse_css_width(width),
  };
  with_registry(state, |registry, collection| {
    registry.register(collection, handle, overrides);
  });
}

/// Unregister from collection (bytes kept for re-add).
#[op2(fast)]
pub fn op_fontdb_remove(state: &OpState, #[smi] handle: u32) {
  with_registry(state, |registry, collection| {
    registry.unregister(collection, handle);
  });
}

/// Drop handle + bytes (FinalizationRegistry; must not throw).
#[op2(fast)]
pub fn op_fontdb_unload(state: &OpState, #[smi] handle: u32) {
  with_registry(state, |registry, collection| {
    registry.unload(collection, handle);
  });
}

#[derive(Serialize)]
struct CssFontQueryResult {
  family: String,
  style: String,
  weight: u16,
  /// font-width percentage (`100` = normal).
  width: f32,
}

/// Parse font shorthand for check()/load(); null if invalid or multi-family.
#[op2]
#[serde]
pub fn op_parse_css_font_query(
  #[string] font: &str,
) -> Option<CssFontQueryResult> {
  let state = parse_css_font(font, &default_font_resolution())?;
  let [family] = <[String; 1]>::try_from(state.families).ok()?;
  Some(CssFontQueryResult {
    family,
    style: style_to_css_str(state.style).to_string(),
    weight: state.weight,
    width: state.width.to_parley().percentage(),
  })
}

/// FontFace weight → `1..=1000`, or `-1` if invalid.
#[op2(fast)]
pub fn op_parse_css_font_weight(#[string] s: &str) -> i32 {
  parse_css_weight(s).map(i32::from).unwrap_or(-1)
}

/// FontFace width/stretch → percentage, or `NaN` if invalid.
#[op2(fast)]
pub fn op_parse_css_font_width(#[string] s: &str) -> f64 {
  parse_css_width(s)
    .map(|w| f64::from(w.percentage()))
    .unwrap_or(f64::NAN)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontFaceMatchInput {
  family: String,
  style: String,
  weight: String,
  width: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FontFaceMatchResult {
  indices: Vec<u32>,
  found: bool,
}

/// CSS font matching for `FontFaceSet.check` / `load` (unicode-range filtered in JS).
/// https://drafts.csswg.org/css-font-loading/#find-the-matching-font-faces
#[op2]
#[serde]
pub fn op_match_font_faces(
  #[string] font: &str,
  #[serde] faces: Vec<FontFaceMatchInput>,
) -> Option<FontFaceMatchResult> {
  let state = parse_css_font(font, &default_font_resolution())?;
  let [family] = <[String; 1]>::try_from(state.families).ok()?;

  if is_generic_family(&family) {
    return Some(FontFaceMatchResult {
      indices: vec![],
      found: true,
    });
  }

  let query_width = state.width.to_parley();
  let query_style = state.style.to_parley();
  let query_weight = FontWeight::new(f32::from(state.weight));

  let mut candidates = Vec::new();
  for (i, face) in faces.iter().enumerate() {
    if face.family != family {
      continue;
    }
    let Some(width) = parse_css_width(&face.width) else {
      continue;
    };
    let Some(style) = parse_css_style(&face.style) else {
      continue;
    };
    let Some(weight) = parse_css_weight(&face.weight) else {
      continue;
    };
    candidates.push(MatchCandidate {
      index: i as u32,
      width: (width.ratio() * 100.0) as i32,
      style: style.to_parley(),
      weight: f32::from(weight),
    });
  }

  if candidates.is_empty() {
    return Some(FontFaceMatchResult {
      indices: vec![],
      found: false,
    });
  }

  let indices =
    match_font_candidates(&candidates, query_width, query_style, query_weight);
  Some(FontFaceMatchResult {
    indices,
    found: true,
  })
}

#[derive(Clone, Copy)]
struct MatchCandidate {
  index: u32,
  /// Percentage points (`100` = normal); fontique scale.
  width: i32,
  style: FontStyle,
  weight: f32,
}

/// Nearest width → style → weight (fontique `match_font`); keeps all winners.
fn match_font_candidates(
  set: &[MatchCandidate],
  width: FontWidth,
  style: FontStyle,
  weight: FontWeight,
) -> Vec<u32> {
  use core::cmp::Ordering::*;

  if set.is_empty() {
    return vec![];
  }
  if set.len() == 1 {
    return vec![set[0].index];
  }

  let mut set: Vec<MatchCandidate> = set.to_vec();
  let width = (width.ratio() * 100.0) as i32;
  let weight = weight.value();

  let use_width = if set.iter().any(|f| f.width == width) {
    width
  } else if width <= 100 {
    set
      .iter()
      .filter(|f| f.width < width)
      .max_by_key(|f| f.width)
      .or_else(|| {
        set
          .iter()
          .filter(|f| f.width > width)
          .min_by_key(|f| f.width)
      })
      .map(|f| f.width)
      .unwrap_or(set[0].width)
  } else {
    set
      .iter()
      .filter(|f| f.width > width)
      .min_by_key(|f| f.width)
      .or_else(|| {
        set
          .iter()
          .filter(|f| f.width < width)
          .max_by_key(|f| f.width)
      })
      .map(|f| f.width)
      .unwrap_or(set[0].width)
  };
  set.retain(|f| f.width == use_width);

  // FontFace style is normal | italic | oblique only (no angle).
  const OBLIQUE_THRESHOLD: f32 = 14.0;
  let use_style = if set.iter().any(|f| f.style == style) {
    style
  } else {
    match style {
      FontStyle::Italic => set
        .iter()
        .find(|f| matches!(f.style, FontStyle::Oblique(_)))
        .map(|f| f.style)
        .unwrap_or(set[0].style),
      FontStyle::Oblique(angle) => {
        let angle = angle.unwrap_or(OBLIQUE_THRESHOLD);
        if set.iter().any(|f| f.style == FontStyle::Italic) && angle >= 0.0 {
          FontStyle::Italic
        } else {
          set
            .iter()
            .find(|f| matches!(f.style, FontStyle::Oblique(_)))
            .or_else(|| set.iter().find(|f| f.style == FontStyle::Italic))
            .map(|f| f.style)
            .unwrap_or(set[0].style)
        }
      }
      FontStyle::Normal => set
        .iter()
        .find(|f| matches!(f.style, FontStyle::Oblique(_)))
        .or_else(|| set.iter().find(|f| f.style == FontStyle::Italic))
        .map(|f| f.style)
        .unwrap_or(set[0].style),
    }
  };
  set.retain(|f| f.style == use_style);

  let use_weight = if let Some(f) = set.iter().find(|f| f.weight == weight) {
    f.weight
  } else if (400.0..=500.0).contains(&weight) {
    set
      .iter()
      .filter(|f| f.weight >= weight && f.weight <= 500.0)
      .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
      .or_else(|| {
        set
          .iter()
          .filter(|f| f.weight < weight)
          .max_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
      })
      .or_else(|| {
        set
          .iter()
          .filter(|f| f.weight > 500.0)
          .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
      })
      .map(|f| f.weight)
      .unwrap_or(set[0].weight)
  } else if weight < 400.0 {
    set
      .iter()
      .filter(|f| f.weight <= weight)
      .max_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
      .or_else(|| {
        set
          .iter()
          .filter(|f| f.weight > weight)
          .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
      })
      .map(|f| f.weight)
      .unwrap_or(set[0].weight)
  } else {
    set
      .iter()
      .filter(|f| f.weight >= weight)
      .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
      .or_else(|| {
        set
          .iter()
          .filter(|f| f.weight < weight)
          .max_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
      })
      .map(|f| f.weight)
      .unwrap_or(set[0].weight)
  };
  set.retain(|f| f.weight == use_weight);

  set.into_iter().map(|f| f.index).collect()
}

#[derive(Serialize)]
struct FontSrcInfo {
  local: bool,
  value: String,
}

/// Parse FontFace `src`; drop unsupported format/tech (e.g. woff/woff2).
#[op2]
#[serde]
pub fn op_parse_css_font_src(#[string] src: &str) -> Option<Vec<FontSrcInfo>> {
  let srcs = parse_css_font_src(src)?;
  Some(
    srcs
      .into_iter()
      .filter(FontSrc::is_supported)
      .map(|src| match src {
        FontSrc::Url { url, .. } => FontSrcInfo {
          local: false,
          value: url,
        },
        FontSrc::Local(value) => FontSrcInfo { local: true, value },
      })
      .collect(),
  )
}

/// Normalize FontFace family (invalid names quoted, not SyntaxError).
/// https://github.com/w3c/csswg-drafts/issues/6236
#[op2]
#[string]
pub fn op_normalize_font_face_family(#[string] family: &str) -> String {
  normalize_font_face_family(family)
}

/// Shared local-font DB + flag set by registerLocalFonts (process-wide).
#[derive(Clone, Default)]
pub struct SharedLocalFontDb {
  inner: Arc<Mutex<SharedLocalFontDbInner>>,
  system_fonts_enabled: Arc<AtomicBool>,
}

impl SharedLocalFontDb {
  pub(crate) fn system_fonts_enabled(&self) -> bool {
    self.system_fonts_enabled.load(Ordering::Relaxed)
  }
}

#[derive(Default)]
struct SharedLocalFontDbInner {
  db: Option<fontdb::Database>,
}

async fn ensure_local_fonts_loaded(shared_db: &SharedLocalFontDb) {
  {
    let inner = shared_db.inner.lock().unwrap();
    if inner.db.is_some() {
      return;
    }
  }
  let db = tokio::task::spawn_blocking(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    db
  })
  .await
  .unwrap_or_else(|_| fontdb::Database::new());
  let mut inner = shared_db.inner.lock().unwrap();
  if inner.db.is_none() {
    inner.db = Some(db);
  }
}

/// Register all installed fonts for canvas + queryLocalFonts.
#[op2(stack_trace)]
pub async fn op_fontdb_register_all_local_fonts(
  state: Rc<RefCell<OpState>>,
) -> Result<(), deno_permissions::PermissionCheckError> {
  let shared_db = {
    let st = state.borrow();
    st.borrow::<SharedLocalFontDb>().clone()
  };

  state
    .borrow_mut()
    .borrow_mut::<PermissionsContainer>()
    .check_sys("localFonts", "Deno.registerLocalFonts")?;

  ensure_local_fonts_loaded(&shared_db).await;

  // Visible to canvas text in this isolate and all workers.
  shared_db
    .system_fonts_enabled
    .store(true, Ordering::Relaxed);
  {
    let state = state.borrow();
    let font_ctx = state.borrow::<Rc<RefCell<FontContext>>>();
    font_ctx.borrow_mut().collection.load_system_fonts();
  }

  Ok(())
}

fn extract_name(face: &ttf_parser::Face, name_id: u16) -> Option<String> {
  face
    .names()
    .into_iter()
    .filter(|name| name.name_id == name_id)
    .find_map(|name| name.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FontDataInfo {
  postscript_name: String,
  full_name: String,
  family: String,
  style: String,
}

#[op2(stack_trace)]
#[serde]
pub async fn op_fontdb_query_local_fonts(
  state: Rc<RefCell<OpState>>,
  #[serde] postscript_names: Option<Vec<String>>,
) -> Result<Vec<FontDataInfo>, deno_permissions::PermissionCheckError> {
  let shared_db = {
    let st = state.borrow();
    st.borrow::<SharedLocalFontDb>().clone()
  };

  state
    .borrow_mut()
    .borrow_mut::<PermissionsContainer>()
    .check_sys("localFonts", "queryLocalFonts")?;

  ensure_local_fonts_loaded(&shared_db).await;

  let filter: Option<HashSet<String>> =
    postscript_names.map(|v| v.into_iter().collect());

  let results = tokio::task::spawn_blocking(move || {
    let inner = shared_db.inner.lock().unwrap();
    let Some(db) = inner.db.as_ref() else {
      return Vec::new();
    };
    let mut results: Vec<FontDataInfo> = Vec::new();
    let mut seen = HashSet::new();

    for face in db.faces() {
      if face.post_script_name.is_empty() {
        continue;
      }
      if let Some(ref filter) = filter
        && !filter.contains(&face.post_script_name)
      {
        continue;
      }
      if !seen.insert(face.post_script_name.clone()) {
        continue;
      }

      let family = face
        .families
        .first()
        .map(|(name, _)| name.clone())
        .unwrap_or_default();

      let (full_name, style) = db
        .with_face_data(face.id, |data, face_index| {
          ttf_parser::Face::parse(data, face_index).ok().map(|f| {
            let full_name = extract_name(&f, ttf_parser::name_id::FULL_NAME)
              .unwrap_or_else(|| family.clone());
            let style = extract_name(&f, ttf_parser::name_id::SUBFAMILY)
              .unwrap_or_else(|| {
                match face.style {
                  fontdb::Style::Normal => "Regular",
                  fontdb::Style::Italic => "Italic",
                  fontdb::Style::Oblique => "Oblique",
                }
                .to_string()
              });
            (full_name, style)
          })
        })
        .flatten()
        .unwrap_or_else(|| {
          (
            family.clone(),
            match face.style {
              fontdb::Style::Normal => "Regular",
              fontdb::Style::Italic => "Italic",
              fontdb::Style::Oblique => "Oblique",
            }
            .to_string(),
          )
        });

      results.push(FontDataInfo {
        postscript_name: face.post_script_name.clone(),
        full_name,
        family,
        style,
      });
    }

    results.sort_by(|a, b| a.postscript_name.cmp(&b.postscript_name));
    results
  })
  .await
  .unwrap_or_default();

  Ok(results)
}

#[op2(stack_trace)]
#[buffer]
pub async fn op_fontdb_local_font_data(
  state: Rc<RefCell<OpState>>,
  #[string] postscript_name: String,
) -> Result<Vec<u8>, JsErrorBox> {
  let shared_db = {
    let st = state.borrow();
    st.borrow::<SharedLocalFontDb>().clone()
  };

  state
    .borrow_mut()
    .borrow_mut::<PermissionsContainer>()
    .check_sys("localFonts", "queryLocalFonts")
    .map_err(JsErrorBox::from_err)?;

  ensure_local_fonts_loaded(&shared_db).await;

  let data = tokio::task::spawn_blocking(move || {
    let inner = shared_db.inner.lock().unwrap();
    let Some(db) = inner.db.as_ref() else {
      return Err(JsErrorBox::generic(format!(
        "Font not found: {postscript_name}"
      )));
    };
    for face in db.faces() {
      if face.post_script_name == postscript_name {
        return db
          .with_face_data(face.id, |data, _| data.to_vec())
          .ok_or_else(|| JsErrorBox::generic("Failed to read font data"));
      }
    }
    Err(JsErrorBox::generic(format!(
      "Font not found: {postscript_name}"
    )))
  })
  .await
  .map_err(|e| JsErrorBox::generic(e.to_string()))??;

  Ok(data)
}

#[cfg(test)]
mod tests {
  use super::*;

  const FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/testdata/NotoSerifCJKjp-Regular-subset.otf"
  ));

  fn collection() -> Collection {
    Collection::new(CollectionOptions {
      shared: false,
      system_fonts: false,
    })
  }

  fn overrides(family: &str) -> FaceOverride {
    FaceOverride {
      family: Some(family.to_string()),
      ..Default::default()
    }
  }

  fn insert(registry: &mut FontRegistry) -> u32 {
    registry.insert_blob(Blob::new(Arc::new(FONT.to_vec())))
  }

  fn face_count(collection: &mut Collection, family: &str) -> usize {
    collection
      .family_by_name(family)
      .map_or(0, |family| family.fonts().len())
  }

  #[test]
  fn register_makes_family_resolvable() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();
    let handle = insert(&mut registry);

    registry.register(&mut collection, handle, overrides("MyFont"));

    assert!(collection.family_id("MyFont").is_some());
    assert_eq!(face_count(&mut collection, "MyFont"), 1);
  }

  /// Removal must drop the family (unregister_font leaves an empty shadow).
  #[test]
  fn unregister_drops_the_family_name() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();
    let handle = insert(&mut registry);
    registry.register(&mut collection, handle, overrides("MyFont"));

    registry.unregister(&mut collection, handle);

    assert!(collection.family_id("MyFont").is_none());
  }

  #[test]
  fn unregister_keeps_other_faces_of_the_same_family() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();
    let first = insert(&mut registry);
    let second = insert(&mut registry);
    registry.register(&mut collection, first, overrides("MyFont"));
    registry.register(&mut collection, second, overrides("MyFont"));
    assert_eq!(face_count(&mut collection, "MyFont"), 2);

    registry.unregister(&mut collection, first);

    assert_eq!(face_count(&mut collection, "MyFont"), 1);
  }

  #[test]
  fn register_is_idempotent() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();
    let handle = insert(&mut registry);

    registry.register(&mut collection, handle, overrides("MyFont"));
    registry.register(&mut collection, handle, overrides("MyFont"));

    assert_eq!(face_count(&mut collection, "MyFont"), 1);
  }

  #[test]
  fn removed_handle_can_be_registered_again() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();
    let handle = insert(&mut registry);
    registry.register(&mut collection, handle, overrides("MyFont"));
    registry.unregister(&mut collection, handle);

    registry.register(&mut collection, handle, overrides("MyFont"));

    assert_eq!(face_count(&mut collection, "MyFont"), 1);
  }

  #[test]
  fn unloaded_handle_cannot_be_registered_again() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();
    let handle = insert(&mut registry);
    registry.register(&mut collection, handle, overrides("MyFont"));
    registry.unload(&mut collection, handle);

    registry.register(&mut collection, handle, overrides("MyFont"));

    assert!(collection.family_id("MyFont").is_none());
  }

  #[test]
  fn unknown_handles_are_a_no_op() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();

    registry.register(&mut collection, 42, overrides("MyFont"));
    registry.unregister(&mut collection, 42);
    registry.unload(&mut collection, 42);

    assert!(collection.family_id("MyFont").is_none());
  }

  #[test]
  fn descriptor_overrides_are_applied() {
    let mut collection = collection();
    let mut registry = FontRegistry::default();
    let handle = insert(&mut registry);

    registry.register(
      &mut collection,
      handle,
      FaceOverride {
        family: Some("MyFont".to_string()),
        style: Some(CssFontStyle::Italic.to_parley()),
        weight: Some(FontWeight::new(700.0)),
        width: Some(CssFontWidth::Condensed.to_parley()),
      },
    );

    let family = collection.family_by_name("MyFont").unwrap();
    let info = family.fonts().first().unwrap();
    assert_eq!(info.style(), CssFontStyle::Italic.to_parley());
    assert_eq!(info.weight(), FontWeight::new(700.0));
    assert_eq!(info.width(), CssFontWidth::Condensed.to_parley());
  }

  #[test]
  fn file_metadata_describes_face_zero() {
    let meta = read_file_metadata(FONT);
    // The fixture's OS/2 table declares usWeightClass 200 despite its
    // "Regular" name, so this also covers weight not being defaulted to 400.
    assert_eq!(meta.weight, 200);
    assert_eq!(meta.style, "normal");
    assert_eq!(meta.width, "normal");
    assert!(!meta.unicode_coverage.is_empty());
    // Ranges are sorted, non-overlapping, and non-empty.
    let mut previous_end = None;
    for [start, end] in meta.unicode_coverage {
      assert!(start <= end);
      if let Some(previous_end) = previous_end {
        assert!(start > previous_end + 1);
      }
      previous_end = Some(end);
    }
  }

  fn cand(index: u32, width_pct: i32, weight: f32) -> MatchCandidate {
    MatchCandidate {
      index,
      width: width_pct,
      style: FontStyle::Normal,
      weight,
    }
  }

  #[test]
  fn match_candidates_prefers_exact_width() {
    let set = [cand(0, 75, 400.0), cand(1, 87, 400.0), cand(2, 100, 400.0)];
    let indices = match_font_candidates(
      &set,
      FontWidth::from_percentage(87.5),
      FontStyle::Normal,
      FontWeight::new(400.0),
    );
    assert_eq!(indices, vec![1]);
  }

  #[test]
  fn match_candidates_nearest_width_below_when_desired_le_100() {
    let set = [cand(0, 75, 400.0), cand(1, 100, 400.0)];
    let indices = match_font_candidates(
      &set,
      FontWidth::SEMI_CONDENSED,
      FontStyle::Normal,
      FontWeight::new(400.0),
    );
    assert_eq!(indices, vec![0]);
  }

  #[test]
  fn match_candidates_nearest_weight_for_bold() {
    let set = [cand(0, 100, 400.0)];
    let indices = match_font_candidates(
      &set,
      FontWidth::NORMAL,
      FontStyle::Normal,
      FontWeight::new(700.0),
    );
    assert_eq!(indices, vec![0]);
  }

  #[test]
  fn match_candidates_keeps_all_with_winning_attrs() {
    let set = [cand(0, 100, 400.0), cand(1, 100, 400.0)];
    let indices = match_font_candidates(
      &set,
      FontWidth::NORMAL,
      FontStyle::Normal,
      FontWeight::new(400.0),
    );
    assert_eq!(indices, vec![0, 1]);
  }
}
