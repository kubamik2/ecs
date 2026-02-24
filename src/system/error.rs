use std::{error::Error, fmt::{Debug, Display}};

use crate::{SystemId, access::AccessError, param::SystemParamError};

#[derive(Clone)]
pub struct InternalSystemError {
    system_name: &'static str,
    system_id: SystemId,
    kind: InternalSystemErrorKind,
}

impl InternalSystemError {
    pub fn system_id(&self) -> &SystemId {
        &self.system_id
    }
}

impl Debug for InternalSystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemInitError")
            .field("system_name", &self.system_name)
            .field("kind", &self.kind)
            .finish()
    }
}

impl Error for InternalSystemError {}

impl InternalSystemError {
    pub fn param(system_name: &'static str, system_id: SystemId, err: SystemParamError) -> Self {
        Self {
            system_name,
            system_id,
            kind: InternalSystemErrorKind::Param(err)
        }
    }

    pub fn access(system_name: &'static str, system_id: SystemId, err: AccessError) -> Self {
        Self {
            system_name,
            system_id,
            kind: InternalSystemErrorKind::Access(err)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum InternalSystemErrorKind {
    Param(SystemParamError),
    Access(AccessError),
}

impl Display for InternalSystemErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Param(param) => std::fmt::Display::fmt(param, f),
            Self::Access(access) => std::fmt::Display::fmt(access, f),
        }
    }
}

impl Display for InternalSystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("system '{}': {}", self.system_name, self.kind))
    }
}
