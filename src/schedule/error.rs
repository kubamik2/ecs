use std::{error::Error, fmt::Display, sync::Arc};

use crate::system::error::InternalSystemError;

#[derive(Clone, Debug)]
pub struct ScheduleRunError {
    label_debug: Option<Arc<str>>,
    kind: ScheduleRunErrorKind
}

impl ScheduleRunError {
    pub(crate) fn different_world(label_debug: Option<Arc<str>>) -> Self {
        Self {
            label_debug,
            kind: ScheduleRunErrorKind::DifferentWorld,
        }
    }

    pub(crate) fn internal_system(label_debug: Option<Arc<str>>, error: InternalSystemError) -> Self {
        Self {
            label_debug,
            kind: ScheduleRunErrorKind::InternalSystem(error),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ScheduleRunErrorKind {
    DifferentWorld,
    InternalSystem(InternalSystemError),
}

impl Display for ScheduleRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label_debug = self.label_debug.as_deref().unwrap_or("unnamed");
        match &self.kind {
            ScheduleRunErrorKind::DifferentWorld => f.write_fmt(format_args!("schedule '{}' ran in a different world", label_debug)),
            ScheduleRunErrorKind::InternalSystem(err) => f.write_fmt(format_args!("schedule '{}', {}", label_debug, err)),
        }
    }
}

impl Error for ScheduleRunError {}
