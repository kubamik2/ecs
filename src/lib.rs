#![feature(sync_unsafe_cell, downcast_unchecked, allocator_api, alloc_layout_extra)]
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
mod tests;
mod storage;

pub use component::{ComponentId, Signature};
pub use world::World;
pub use query::Query;
pub use resource::{Res, ResMut, ResourceId};
pub use derive::{Component, Resource, Event, ScheduleLabel};
pub use schedule::{Schedule, ScheduleLabel};
pub use system::{Commands, SystemHandle, SystemValidationError, SystemInput, IntoSystem, SystemId};
pub use signal::Signal;
pub use event::{Event, EventReader, EventReadWriter, EventQueue};
pub use entity::{Entity, EntityBundle};
pub use observer::ObserverInput;

pub trait Component: Send + Sync + 'static {}
pub trait Resource: Send + Sync + 'static {}
