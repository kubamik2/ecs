#![feature(sync_unsafe_cell, downcast_unchecked, allocator_api)]
mod bitmap;
mod component;
mod entity;
pub mod system;
mod query;
pub mod param;
pub mod access;
mod resource;
pub mod schedule;
mod trigger;
mod event;
mod observer;
mod world;
#[cfg(test)]
mod tests;
mod storage;
pub mod error;

pub use component::{ComponentId, Signature, ComponentBundle, Component};
pub use world::{World, WorldResMut};
pub use query::{Query, QueryData, Without, With, QueryFilter, Children};
pub use resource::{Res, ResMut, ResourceId, Changed, Resource};
pub use derive::{Component, Resource, ScheduleLabel};
pub use schedule::{Schedule, ScheduleLabel};
pub use system::{Commands, SystemHandle, SystemInput, SystemOutput, IntoSystem, SystemId, Local, System};
pub use trigger::Trigger;
pub use event::{EventReader, EventReadWriter, EventQueue, EventReaderState, EventIterator};
pub use entity::Entity;
pub use observer::{ObserverInput, TriggerInput};
