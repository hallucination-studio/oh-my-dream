//! Concrete Desktop adapters for production generation-provider routes.

mod fal;
mod fal_image_to_video;
mod fal_text_to_image;

pub use fal_image_to_video::FalImageToVideoProviderRouteImpl;
pub use fal_text_to_image::FalTextToImageProviderRouteImpl;
