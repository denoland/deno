use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum CssColor {
  CurrentColor(),
  #[serde(rename = "rgba")]
  RGBA(RGBA),
  // TODO(lucacasonato): others
}

impl From<parcel_css::values::color::CssColor> for CssColor {
  fn from(color: parcel_css::values::color::CssColor) -> Self {
    match color {
      parcel_css::values::color::CssColor::CurrentColor => {
        CssColor::CurrentColor()
      }
      parcel_css::values::color::CssColor::RGBA(rgba) => {
        CssColor::RGBA(rgba.into())
      }
      _ => todo!(),
    }
  }
}

#[derive(Serialize)]
pub struct RGBA {
  pub red: u8,
  pub green: u8,
  pub blue: u8,
  pub alpha: u8,
}

impl From<cssparser::RGBA> for RGBA {
  fn from(color: cssparser::RGBA) -> Self {
    RGBA {
      red: color.red,
      green: color.green,
      blue: color.blue,
      alpha: color.alpha,
    }
  }
}
