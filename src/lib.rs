// #![warn(clippy::all)]
// #![warn(clippy::pedantic)]
#![warn(rust_2018_idioms)]

pub mod engine;
pub mod event;
pub mod math;
pub mod world;

pub use engine::Engine;
pub use event::Event;
pub use world::{BodyHandle, BodyView, World};
