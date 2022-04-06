use serde::Serialize;

use crate::values::CssColor;

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum Property {
  BackgroundColor(CssColor),
}

impl<'a> From<parcel_css::properties::Property<'a>> for Property {
  fn from(property: parcel_css::properties::Property) -> Self {
    match property {
      parcel_css::properties::Property::BackgroundColor(color) => {
        Property::BackgroundColor(color.into())
      }
      _ => todo!(),
    }
  }
}
