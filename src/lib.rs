#![feature(sync_unsafe_cell, downcast_unchecked)]
mod bitmap;
mod component;
mod entity;
mod system;
mod query;
mod param;
mod access;
mod resource;
mod sparse_set;
mod schedule;
mod signal;
mod event;
mod observer;
mod world;
mod tests;

pub use component::{ComponentId, Signature};
pub use world::World;
pub use query::Query;
pub use resource::{Res, ResMut};
pub use derive::{Component, Resource, Event};
pub use schedule::{Schedule, Schedules};
pub use system::Commands;
pub use signal::Signal;
pub use event::{Event, EventReader, EventReadWriter, EventQueue};
pub use entity::Entity;

pub const MAX_ENTITIES: usize = sparse_set::SPARSE_SET_CAPACITY;
pub const MAX_COMPONENTS: usize = 128;


pub trait Component: Send + Sync + 'static {}
pub trait Resource: Send + Sync + 'static {}
