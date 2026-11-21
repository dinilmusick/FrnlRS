//! Runtime-neutral semantic frontend state and control invocation.
//! Applications supply their own real renderer/event callbacks; this crate never renders,
//! fabricates a UI event, or makes an inaccessible control callable.
//!
//! With default features disabled the semantic runtime uses only core + alloc.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Control {
    Click,
    Input,
    Change,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    None,
    Text(String),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceError {
    EmptyId,
    Duplicate(String),
    Unknown(String),
    Cycle(String),
    Unavailable(String),
    InvalidPayload,
    Handler(String),
}

impl SurfaceError {
    /// Stable error code for native control adapters and logs.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyId => "FRNL_SURFACE_EMPTY_ID",
            Self::Duplicate(_) => "FRNL_SURFACE_DUPLICATE",
            Self::Unknown(_) => "FRNL_SURFACE_UNKNOWN",
            Self::Cycle(_) => "FRNL_SURFACE_CYCLE",
            Self::Unavailable(_) => "FRNL_SURFACE_UNAVAILABLE",
            Self::InvalidPayload => "FRNL_SURFACE_INVALID_PAYLOAD",
            Self::Handler(_) => "FRNL_SURFACE_HANDLER",
        }
    }
}
impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {self:?}", self.code())
    }
}

impl core::error::Error for SurfaceError {}

#[derive(Clone, Debug)]
pub struct Descriptor {
    pub id: String,
    pub parent_id: Option<String>,
    pub visible: bool,
    pub enabled: bool,
    pub read_only: bool,
    pub controls: BTreeSet<Control>,
}

impl Descriptor {
    pub fn new(id: impl Into<String>, controls: impl IntoIterator<Item = Control>) -> Self {
        Self {
            id: id.into(),
            parent_id: None,
            visible: true,
            enabled: true,
            read_only: false,
            controls: controls.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub id: String,
    pub parent_id: Option<String>,
    pub visible: bool,
    pub enabled: bool,
    pub read_only: bool,
    pub controls: Vec<Control>,
}

type Handler = Box<dyn FnMut(Control, &Value) -> Result<(), SurfaceError>>;

struct Mounted {
    descriptor: Descriptor,
    handler: Handler,
}

#[derive(Default)]
pub struct Surface {
    mounted: BTreeMap<String, Mounted>,
}

impl Surface {
    /// Only registered renderer components can be inspected or invoked.
    /// The application handler is the same event path used by its native UI.
    pub fn mount(
        &mut self,
        descriptor: Descriptor,
        handler: impl FnMut(Control, &Value) -> Result<(), SurfaceError> + 'static,
    ) -> Result<(), SurfaceError> {
        if descriptor.id.trim().is_empty() {
            return Err(SurfaceError::EmptyId);
        }
        if self.mounted.contains_key(&descriptor.id) {
            return Err(SurfaceError::Duplicate(descriptor.id));
        }
        if let Some(parent) = &descriptor.parent_id {
            if !self.mounted.contains_key(parent) {
                return Err(SurfaceError::Unknown(parent.clone()));
            }
        }
        self.mounted.insert(
            descriptor.id.clone(),
            Mounted {
                descriptor,
                handler: Box::new(handler),
            },
        );
        Ok(())
    }

    /// Unmounting a parent preserves live descendants but detaches direct children.
    pub fn unmount(&mut self, id: &str) -> Result<(), SurfaceError> {
        if self.mounted.remove(id).is_none() {
            return Err(SurfaceError::Unknown(id.to_owned()));
        }
        for mounted in self.mounted.values_mut() {
            if mounted.descriptor.parent_id.as_deref() == Some(id) {
                mounted.descriptor.parent_id = None;
            }
        }
        Ok(())
    }

    /// Reject cycles and dangling parents before committing the new relationship.
    pub fn set_parent(&mut self, id: &str, parent: Option<&str>) -> Result<(), SurfaceError> {
        if !self.mounted.contains_key(id) {
            return Err(SurfaceError::Unknown(id.to_owned()));
        }
        let mut cursor = parent.map(str::to_owned);
        while let Some(current) = cursor {
            if current == id {
                return Err(SurfaceError::Cycle(id.to_owned()));
            }
            let ancestor = self
                .mounted
                .get(&current)
                .ok_or_else(|| SurfaceError::Unknown(current.clone()))?;
            cursor = ancestor.descriptor.parent_id.clone();
        }
        if let Some(component) = self.mounted.get_mut(id) {
            component.descriptor.parent_id = parent.map(str::to_owned);
        }
        Ok(())
    }

    /// Called when the actual native renderer changes effective element state.
    pub fn update_availability(
        &mut self,
        id: &str,
        visible: bool,
        enabled: bool,
        read_only: bool,
    ) -> Result<(), SurfaceError> {
        let component = self
            .mounted
            .get_mut(id)
            .ok_or_else(|| SurfaceError::Unknown(id.to_owned()))?;
        component.descriptor.visible = visible;
        component.descriptor.enabled = enabled;
        component.descriptor.read_only = read_only;
        Ok(())
    }

    fn effective(&self, id: &str) -> Result<(bool, bool), SurfaceError> {
        let mut cursor = Some(id.to_owned());
        let mut seen = BTreeSet::new();
        let (mut visible, mut enabled) = (true, true);
        while let Some(current) = cursor {
            if !seen.insert(current.clone()) {
                return Err(SurfaceError::Cycle(id.to_owned()));
            }
            let component = self
                .mounted
                .get(&current)
                .ok_or_else(|| SurfaceError::Unknown(current.clone()))?;
            visible &= component.descriptor.visible;
            enabled &= component.descriptor.enabled;
            cursor = component.descriptor.parent_id.clone();
        }
        Ok((visible, enabled))
    }

    /// No snapshot is synthesized for an unmounted component.
    pub fn inspect(&self) -> Result<Vec<Snapshot>, SurfaceError> {
        self.mounted
            .values()
            .map(|node| {
                let desc = &node.descriptor;
                let (visible, enabled) = self.effective(&desc.id)?;
                let controls = if visible && enabled {
                    desc.controls
                        .iter()
                        .copied()
                        .filter(|control| !desc.read_only || matches!(control, Control::Click))
                        .collect()
                } else {
                    Vec::new()
                };
                Ok(Snapshot {
                    id: desc.id.clone(),
                    parent_id: desc.parent_id.clone(),
                    visible,
                    enabled,
                    read_only: desc.read_only,
                    controls,
                })
            })
            .collect()
    }

    /// Invoke the registered application callback, never a synthetic success.
    pub fn invoke(
        &mut self,
        id: &str,
        control: Control,
        payload: Value,
    ) -> Result<(), SurfaceError> {
        let (visible, enabled) = self.effective(id)?;
        let node = self
            .mounted
            .get_mut(id)
            .ok_or_else(|| SurfaceError::Unknown(id.to_owned()))?;
        if !visible
            || !enabled
            || !node.descriptor.controls.contains(&control)
            || (node.descriptor.read_only && control != Control::Click)
        {
            return Err(SurfaceError::Unavailable(id.to_owned()));
        }
        match (&control, &payload) {
            (Control::Click, Value::None)
            | (Control::Input | Control::Change, Value::Text(_) | Value::Bool(_)) => {}
            _ => return Err(SurfaceError::InvalidPayload),
        }
        (node.handler)(control, &payload)
    }
}

/// Headless FrnlTS semantic runtime with host-provided platform adapters.
pub mod semantic;
