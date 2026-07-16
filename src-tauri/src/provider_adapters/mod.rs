//! Concrete Desktop adapters for production generation-provider routes.

mod elevenlabs_text_to_speech;
mod fal;
mod fal_image_to_video;
mod fal_text_to_image;

pub use elevenlabs_text_to_speech::{ElevenLabsTextToSpeechProviderRouteImpl, ElevenLabsVoiceId};
pub use fal_image_to_video::FalImageToVideoProviderRouteImpl;
pub use fal_text_to_image::FalTextToImageProviderRouteImpl;
