// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use deno_permissions::PermissionsContainer;
use serde::Serialize;

/// Local font metadata shared across all workers.
/// Populated once by `Deno.loadLocalFonts()`, then each worker copies
/// the FaceInfo entries into its own FontSystem via `push_face_info`.
#[derive(Clone, Default)]
pub struct SharedLocalFontDb(Arc<Mutex<SharedLocalFontDbInner>>);

#[derive(Default)]
struct SharedLocalFontDbInner {
  db: Option<fontdb::Database>,
}

async fn ensure_local_fonts_loaded(shared_db: &SharedLocalFontDb) {
  {
    let inner = shared_db.0.lock().unwrap();
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
  let mut inner = shared_db.0.lock().unwrap();
  if inner.db.is_none() {
    inner.db = Some(db);
  }
}

#[op2(stack_trace)]
pub async fn op_fontdb_load_local_fonts(
  state: Rc<RefCell<OpState>>,
) -> Result<(), deno_permissions::PermissionCheckError> {
  let shared_db = {
    let st = state.borrow();
    st.borrow::<SharedLocalFontDb>().clone()
  };

  state
    .borrow_mut()
    .borrow_mut::<PermissionsContainer>()
    .check_sys("localFonts", "Deno.loadLocalFonts")?;

  ensure_local_fonts_loaded(&shared_db).await;

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
    let inner = shared_db.0.lock().unwrap();
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
    let inner = shared_db.0.lock().unwrap();
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
