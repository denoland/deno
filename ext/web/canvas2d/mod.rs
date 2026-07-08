// Copyright 2018-2026 the Deno authors. MIT license.

mod context;
mod error;
mod filter;
pub mod gradient;
mod image;
mod path;
pub mod pattern;
pub mod text_metrics;

pub use context::CONTEXT_ID;
pub use context::OffscreenCanvasRenderingContext2D;
pub use context::UNSTABLE_FEATURE_NAME;
pub use context::create_context;
pub(crate) use context::init_canvas_renderer;
pub use context::op_canvas2d_init;
pub use error::Canvas2DError;
pub use filter::CanvasFilter;
pub use gradient::CanvasGradient;
pub use image::set_offscreen_canvas_pixel_sync;
pub use path::Path2D;
pub use pattern::CanvasPattern;
pub use text_metrics::TextMetrics;
