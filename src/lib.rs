#![feature(sync_unsafe_cell, downcast_unchecked, allocator_api, alloc_layout_extra, trait_alias)]
mod bitmap;
mod component;
mod entity;
mod system;
mod query;
mod param;
mod access;
mod resource;
mod schedule;
mod signal;
mod event;
mod observer;
mod world;
#[cfg(test)]
mod tests;
mod storage;

pub use component::{ComponentId, Signature};
pub use world::{World, WorldResMut};
pub use query::{Query, QueryData};
pub use resource::{Res, ResMut, ResourceId};
pub use derive::{Component, Resource, ScheduleLabel};
pub use schedule::{Schedule, ScheduleLabel};
pub use system::{Commands, SystemHandle, SystemInput, IntoSystem, SystemId};
pub use signal::Signal;
pub use event::{Event, EventReader, EventReadWriter, EventQueue, EventReaderState};
pub use entity::{Entity, EntityBundle};
pub use observer::{ObserverInput, SignalInput};

pub trait Component: Send + Sync + 'static {}
pub trait Resource: Send + Sync + 'static {}
