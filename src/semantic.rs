//! Portable FrnlTS semantic core. Rendering, I/O, time, and model inference belong to hosts.
//! All ordered collections and event queues are deterministic for a given command sequence.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

pub type Properties = BTreeMap<String, Data>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Data {
    Null,
    Bool(bool),
    Integer(i64),
    Text(String),
    List(Vec<Data>),
    Object(Properties),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataKind {
    Any,
    Null,
    Bool,
    Integer,
    Text,
    List,
    Object,
}

impl DataKind {
    pub fn accepts(self, value: &Data) -> bool {
        matches!(
            (self, value),
            (Self::Any, _)
                | (Self::Null, Data::Null)
                | (Self::Bool, Data::Bool(_))
                | (Self::Integer, Data::Integer(_))
                | (Self::Text, Data::Text(_))
                | (Self::List, Data::List(_))
                | (Self::Object, Data::Object(_))
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid(String),
    Duplicate(String),
    Missing(String),
    WrongState(String),
    WrongPayload(String),
    Cycle(String),
    Unavailable(String),
    Adapter(String),
    Handler(String),
    TimeReversed,
    LimitExceeded,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

fn nonempty(s: &str) -> Result<(), Error> {
    if s.trim().is_empty() {
        Err(Error::Invalid("identifier must be nonempty".into()))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub application: Option<String>,
    pub workspace: Option<String>,
    pub page: Option<String>,
    pub section: Option<String>,
    pub form: Option<String>,
    pub locale: Option<String>,
    pub fields: Properties,
}
impl Context {
    pub fn merge(&self, child: &Self) -> Self {
        let mut fields = self.fields.clone();
        fields.extend(child.fields.clone());
        Self {
            application: child
                .application
                .clone()
                .or_else(|| self.application.clone()),
            workspace: child.workspace.clone().or_else(|| self.workspace.clone()),
            page: child.page.clone().or_else(|| self.page.clone()),
            section: child.section.clone().or_else(|| self.section.clone()),
            form: child.form.clone().or_else(|| self.form.clone()),
            locale: child.locale.clone().or_else(|| self.locale.clone()),
            fields,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyframe {
    pub at_ms: u16,
    pub value: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visual {
    Fixed(Data),
    Curve(Vec<Keyframe>),
}
impl Visual {
    fn validate(&self) -> Result<(), Error> {
        if let Self::Curve(points) = self {
            if points.len() < 2
                || points.first().map(|v| v.at_ms) != Some(0)
                || points.last().map(|v| v.at_ms) != Some(1000)
                || points.windows(2).any(|w| w[0].at_ms >= w[1].at_ms)
            {
                return Err(Error::Invalid(
                    "curve must span 0..1000ms with increasing keys".into(),
                ));
            }
        }
        Ok(())
    }
    fn at(&self, time: u16) -> Data {
        match self {
            Self::Fixed(value) => value.clone(),
            Self::Curve(points) => {
                for pair in points.windows(2) {
                    if time <= pair[1].at_ms {
                        let a = &pair[0];
                        let b = &pair[1];
                        let numerator = (time.saturating_sub(a.at_ms) as i128)
                            * (b.value as i128 - a.value as i128);
                        let value = a.value as i128 + numerator / (b.at_ms - a.at_ms) as i128;
                        return Data::Integer(
                            value.clamp(i64::MIN as i128, i64::MAX as i128) as i64
                        );
                    }
                }
                Data::Integer(points.last().map_or(0, |point| point.value))
            }
        }
    }
}

pub type Visuals = BTreeMap<String, Visual>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhaseDefinitions {
    pub attack_ms: Option<u64>,
    pub sustain: Visuals,
    pub attack: Visuals,
    pub release_ms: Option<u64>,
    pub release: Visuals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDefinition {
    pub function_id: String,
    pub payload: DataKind,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateDefinition {
    pub events: BTreeMap<String, EventDefinition>,
    pub visuals: Visuals,
    pub phases: PhaseDefinitions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentDefinition {
    pub name: String,
    pub initial_state: String,
    pub states: BTreeMap<String, StateDefinition>,
    pub context: Context,
}

impl ComponentDefinition {
    pub fn validate(&self) -> Result<(), Error> {
        nonempty(&self.name)?;
        nonempty(&self.initial_state)?;
        if self.states.is_empty() || !self.states.contains_key(&self.initial_state) {
            return Err(Error::Invalid("initial state must be defined".into()));
        }
        for (state_name, state) in &self.states {
            nonempty(state_name)?;
            for (event, definition) in &state.events {
                nonempty(event)?;
                nonempty(&definition.function_id)?;
            }
            for visual in state
                .visuals
                .values()
                .chain(state.phases.sustain.values())
                .chain(state.phases.attack.values())
                .chain(state.phases.release.values())
            {
                visual.validate()?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Attack,
    Sustain,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecyclePhase {
    Created,
    Initializing,
    Mounted,
    Active,
    Suspended,
    Resuming,
    Disposing,
    Disposed,
    Failed,
}
impl LifecyclePhase {
    pub fn can_transition(self, to: Self) -> bool {
        use LifecyclePhase::*;
        self == to
            || matches!(
                (self, to),
                (Created, Initializing | Disposing)
                    | (Initializing, Mounted | Failed | Disposing)
                    | (Mounted, Active | Suspended | Disposing | Failed)
                    | (Active, Suspended | Disposing | Failed)
                    | (Suspended, Resuming | Disposing | Failed)
                    | (Resuming, Active | Failed | Disposing)
                    | (Disposing, Disposed)
                    | (Failed, Initializing | Disposing | Disposed)
            )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub definition: String,
    pub parent: Option<String>,
    pub state: String,
    pub pending_state: Option<String>,
    pub phase: Phase,
    pub phase_started_ms: u64,
    pub lifecycle: LifecyclePhase,
    pub properties: Properties,
    pub data: Properties,
    pub overrides: Properties,
    pub theme: Option<String>,
    pub context: Context,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Theme {
    // Component definition name -> state name -> visual parameters.
    pub components: BTreeMap<String, BTreeMap<String, Visuals>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRecord {
    pub sequence: u64,
    pub component: String,
    pub path: Vec<String>,
    pub state: String,
    pub event: String,
    pub function: String,
    pub payload: Data,
    pub result: Data,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateChange {
    pub component: String,
    pub from: String,
    pub to: String,
    pub at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub id: String,
    pub path: Vec<String>,
    pub definition: String,
    pub state: String,
    pub pending_state: Option<String>,
    pub phase: Phase,
    pub phase_elapsed_ms: u64,
    pub lifecycle: LifecyclePhase,
    pub visuals: Properties,
    pub properties: Properties,
    pub data: Properties,
    pub context: Context,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    SetState {
        component: String,
        state: String,
    },
    SetVisual {
        component: String,
        key: String,
        value: Data,
    },
    SetData {
        component: String,
        key: String,
        value: Data,
    },
    Emit {
        component: String,
        event: String,
        payload: Data,
    },
    Flow(String),
}

#[derive(Clone, Debug)]
pub struct ActionContext {
    pub component: String,
    pub state: String,
    actions: Vec<Action>,
}
impl ActionContext {
    fn new(component: &str, state: &str) -> Self {
        Self {
            component: component.into(),
            state: state.into(),
            actions: Vec::new(),
        }
    }
    pub fn set_state(&mut self, state: impl Into<String>) {
        self.actions.push(Action::SetState {
            component: self.component.clone(),
            state: state.into(),
        });
    }
    pub fn emit(&mut self, event: impl Into<String>, payload: Data) {
        self.actions.push(Action::Emit {
            component: self.component.clone(),
            event: event.into(),
            payload,
        });
    }
    pub fn send(&mut self, component: impl Into<String>, event: impl Into<String>, payload: Data) {
        self.actions.push(Action::Emit {
            component: component.into(),
            event: event.into(),
            payload,
        });
    }
    pub fn set_visual(&mut self, key: impl Into<String>, value: Data) {
        self.actions.push(Action::SetVisual {
            component: self.component.clone(),
            key: key.into(),
            value,
        });
    }
    pub fn set_data(&mut self, key: impl Into<String>, value: Data) {
        self.actions.push(Action::SetData {
            component: self.component.clone(),
            key: key.into(),
            value,
        });
    }
    pub fn flow(&mut self, name: impl Into<String>) {
        self.actions.push(Action::Flow(name.into()));
    }
}

type Handler = Box<dyn FnMut(&Data, &mut ActionContext) -> Result<Data, Error>>;
type Listener = Box<dyn FnMut(&EventRecord, &mut ActionContext) -> Result<(), Error>>;
type EventKey = (String, String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowStep {
    SetState {
        component: String,
        state: String,
    },
    SetVisual {
        component: String,
        key: String,
        value: Data,
    },
    SetData {
        component: String,
        key: String,
        value: Data,
    },
    Emit {
        component: String,
        event: String,
        payload: Data,
    },
    Call {
        component: String,
        function: String,
        payload: Data,
    },
    Flow(String),
    IfEqual {
        component: String,
        key: String,
        value: Data,
        then_steps: Vec<FlowStep>,
        else_steps: Vec<FlowStep>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    pub id: String,
    pub steps: Vec<FlowStep>,
}

pub trait Renderer {
    fn render(&mut self, snapshots: &[Snapshot]) -> Result<(), Error>;
}
pub trait Clock {
    fn now_ms(&self) -> Result<u64, Error>;
}
pub trait EventSource {
    fn next_event(&mut self) -> Result<Option<(String, String, Data)>, Error>;
}
pub trait Storage {
    fn store(&mut self, namespace: &str, snapshot: &[Snapshot]) -> Result<(), Error>;
}
pub trait ExternalIo {
    fn invoke(&mut self, name: &str, payload: &Data) -> Result<Data, Error>;
}
pub trait ModelAdapter {
    fn resolve(
        &mut self,
        input: &str,
        contract: &InputContract,
        context: &Context,
    ) -> Result<Data, Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputContract {
    pub name: String,
    pub kind: DataKind,
    pub vocabulary: Vec<String>,
    pub allow_raw_text: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedInput {
    pub value: Data,
    pub provider: String,
}

#[derive(Default)]
pub struct Runtime {
    definitions: BTreeMap<String, ComponentDefinition>,
    entities: BTreeMap<String, Entity>,
    handlers: BTreeMap<(String, String), Handler>,
    listeners: BTreeMap<u64, (EventKey, Listener)>,
    functions: BTreeMap<String, Handler>,
    themes: BTreeMap<String, Theme>,
    libraries: Properties,
    page_data: BTreeMap<String, Properties>,
    flows: BTreeMap<String, Flow>,
    flow_bindings: BTreeMap<EventKey, Vec<String>>,
    queue: VecDeque<(String, String, Data)>,
    dispatching: bool,
    next_listener: u64,
    now_ms: u64,
    history: Vec<EventRecord>,
    state_changes: Vec<StateChange>,
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }
    pub fn history(&self) -> &[EventRecord] {
        &self.history
    }
    pub fn state_changes(&self) -> &[StateChange] {
        &self.state_changes
    }

    pub fn register_definition(&mut self, definition: ComponentDefinition) -> Result<(), Error> {
        definition.validate()?;
        if self.definitions.contains_key(&definition.name) {
            return Err(Error::Duplicate(definition.name));
        }
        for theme in self.themes.values() {
            Self::validate_theme_for(theme, &definition)?;
        }
        self.definitions.insert(definition.name.clone(), definition);
        Ok(())
    }

    fn validate_theme_for(theme: &Theme, definition: &ComponentDefinition) -> Result<(), Error> {
        if let Some(states) = theme.components.get(&definition.name) {
            for (state, visuals) in states {
                if !definition.states.contains_key(state) {
                    return Err(Error::Invalid(format!(
                        "theme references unknown state: {state}"
                    )));
                }
                for visual in visuals.values() {
                    visual.validate()?;
                }
            }
        }
        Ok(())
    }

    pub fn register_theme(&mut self, name: &str, theme: Theme) -> Result<(), Error> {
        nonempty(name)?;
        for definition in self.definitions.values() {
            Self::validate_theme_for(&theme, definition)?;
        }
        for states in theme.components.values() {
            for visuals in states.values() {
                for visual in visuals.values() {
                    visual.validate()?;
                }
            }
        }
        if self.themes.contains_key(name) {
            return Err(Error::Duplicate(name.into()));
        }
        self.themes.insert(name.into(), theme);
        Ok(())
    }

    pub fn mount(&mut self, id: &str, definition: &str, parent: Option<&str>) -> Result<(), Error> {
        nonempty(id)?;
        if self.entities.contains_key(id) {
            return Err(Error::Duplicate(id.into()));
        }
        if let Some(p) = parent {
            if !self.entities.contains_key(p) {
                return Err(Error::Missing(p.into()));
            }
        }
        let spec = self
            .definitions
            .get(definition)
            .ok_or_else(|| Error::Missing(definition.into()))?;
        let initial = &spec.states[&spec.initial_state];
        let phase = if initial.phases.attack_ms.unwrap_or(0) > 0 {
            Phase::Attack
        } else {
            Phase::Sustain
        };
        self.entities.insert(
            id.into(),
            Entity {
                definition: definition.into(),
                parent: parent.map(str::to_owned),
                state: spec.initial_state.clone(),
                pending_state: None,
                phase,
                phase_started_ms: self.now_ms,
                lifecycle: LifecyclePhase::Created,
                properties: Properties::new(),
                data: Properties::new(),
                overrides: Properties::new(),
                theme: None,
                context: spec.context.clone(),
            },
        );
        Ok(())
    }

    pub fn unmount(&mut self, id: &str) -> Result<(), Error> {
        if self.entities.remove(id).is_none() {
            return Err(Error::Missing(id.into()));
        }
        for entity in self.entities.values_mut() {
            if entity.parent.as_deref() == Some(id) {
                entity.parent = None;
            }
        }
        self.handlers.retain(|(c, _), _| c != id);
        self.listeners.retain(|_, ((c, _), _)| c != id);
        self.flow_bindings.retain(|(c, _), _| c != id);
        self.queue.retain(|(c, _, _)| c != id);
        Ok(())
    }

    pub fn set_parent(&mut self, id: &str, parent: Option<&str>) -> Result<(), Error> {
        self.entity(id)?;
        if let Some(p) = parent {
            self.path(p)?;
            if p == id || self.path(p)?.iter().any(|part| part == id) {
                return Err(Error::Cycle(id.into()));
            }
        }
        self.entities.get_mut(id).expect("validated entity").parent = parent.map(str::to_owned);
        Ok(())
    }

    pub fn entity(&self, id: &str) -> Result<&Entity, Error> {
        self.entities
            .get(id)
            .ok_or_else(|| Error::Missing(id.into()))
    }
    pub fn path(&self, id: &str) -> Result<Vec<String>, Error> {
        let mut path = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = Some(id);
        while let Some(name) = current {
            if !seen.insert(name.to_owned()) {
                return Err(Error::Cycle(name.into()));
            }
            path.push(name.to_owned());
            current = self.entity(name)?.parent.as_deref();
        }
        path.reverse();
        Ok(path)
    }
    pub fn check_path(&self, path: &[String]) -> Result<(), Error> {
        if path.is_empty() {
            return Err(Error::Invalid("path is empty".into()));
        }
        if self.path(path.last().expect("nonempty"))? != path {
            return Err(Error::Invalid("path does not match current scope".into()));
        }
        Ok(())
    }

    pub fn context(&self, id: &str) -> Result<Context, Error> {
        let mut resolved = Context::default();
        for part in self.path(id)? {
            resolved = resolved.merge(&self.entity(&part)?.context);
        }
        Ok(resolved)
    }
    pub fn set_context(&mut self, id: &str, context: Context) -> Result<(), Error> {
        self.entities
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.into()))?
            .context = context;
        Ok(())
    }
    pub fn set_data(&mut self, id: &str, key: &str, value: Data) -> Result<(), Error> {
        nonempty(key)?;
        self.entities
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.into()))?
            .data
            .insert(key.into(), value);
        Ok(())
    }
    pub fn set_property(&mut self, id: &str, key: &str, value: Data) -> Result<(), Error> {
        nonempty(key)?;
        self.entities
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.into()))?
            .properties
            .insert(key.into(), value);
        Ok(())
    }
    pub fn set_page_data(&mut self, page: &str, key: &str, value: Data) -> Result<(), Error> {
        nonempty(page)?;
        nonempty(key)?;
        self.page_data
            .entry(page.into())
            .or_default()
            .insert(key.into(), value);
        Ok(())
    }
    pub fn page_data(&self, page: &str) -> Option<&Properties> {
        self.page_data.get(page)
    }

    pub fn register_library(&mut self, address: &str, value: Data) -> Result<(), Error> {
        nonempty(address)?;
        if self.libraries.contains_key(address) {
            return Err(Error::Duplicate(address.into()));
        }
        self.libraries.insert(address.into(), value);
        Ok(())
    }
    pub fn library(&self, address: &str) -> Result<&Data, Error> {
        self.libraries
            .get(address)
            .ok_or_else(|| Error::Missing(address.into()))
    }
    pub fn unregister_library(&mut self, address: &str) -> Option<Data> {
        self.libraries.remove(address)
    }
    pub fn set_visual(&mut self, id: &str, key: &str, value: Data) -> Result<(), Error> {
        nonempty(key)?;
        self.entities
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.into()))?
            .overrides
            .insert(key.into(), value);
        Ok(())
    }
    pub fn clear_visuals(&mut self, id: &str) -> Result<(), Error> {
        self.entities
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.into()))?
            .overrides
            .clear();
        Ok(())
    }
    pub fn apply_theme(&mut self, id: &str, theme: &str) -> Result<(), Error> {
        if !self.themes.contains_key(theme) {
            return Err(Error::Missing(theme.into()));
        }
        let entity = self
            .entities
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.into()))?;
        entity.theme = Some(theme.into());
        entity.overrides.clear();
        Ok(())
    }
    pub fn transition_lifecycle(&mut self, id: &str, to: LifecyclePhase) -> Result<(), Error> {
        let entity = self
            .entities
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.into()))?;
        if !entity.lifecycle.can_transition(to) {
            return Err(Error::WrongState(format!(
                "{:?} -> {to:?}",
                entity.lifecycle
            )));
        }
        entity.lifecycle = to;
        Ok(())
    }

    fn state_definition(&self, id: &str) -> Result<&StateDefinition, Error> {
        let entity = self.entity(id)?;
        self.definitions
            .get(&entity.definition)
            .and_then(|def| def.states.get(&entity.state))
            .ok_or_else(|| Error::WrongState(id.into()))
    }
    fn enter_state(&mut self, id: &str, state: &str, at: u64) -> Result<(), Error> {
        let entity = self.entity(id)?;
        let definition = self
            .definitions
            .get(&entity.definition)
            .ok_or_else(|| Error::Missing(entity.definition.clone()))?;
        let next = definition
            .states
            .get(state)
            .ok_or_else(|| Error::WrongState(state.into()))?;
        let phase = if next.phases.attack_ms.unwrap_or(0) > 0 {
            Phase::Attack
        } else {
            Phase::Sustain
        };
        let previous = entity.state.clone();
        let item = self.entities.get_mut(id).expect("validated entity");
        item.state = state.into();
        item.phase = phase;
        item.pending_state = None;
        item.phase_started_ms = at;
        if previous != state {
            self.state_changes.push(StateChange {
                component: id.into(),
                from: previous,
                to: state.into(),
                at_ms: at,
            });
        }
        Ok(())
    }

    pub fn set_state(&mut self, id: &str, next: &str) -> Result<(), Error> {
        nonempty(next)?;
        let entity = self.entity(id)?;
        let definition = self
            .definitions
            .get(&entity.definition)
            .ok_or_else(|| Error::Missing(entity.definition.clone()))?;
        if !definition.states.contains_key(next) {
            return Err(Error::WrongState(next.into()));
        }
        if entity.state == next && entity.pending_state.is_none() {
            return Ok(());
        }
        let release = definition.states[&entity.state]
            .phases
            .release_ms
            .unwrap_or(0);
        if release == 0 {
            self.enter_state(id, next, self.now_ms)
        } else {
            let item = self.entities.get_mut(id).expect("validated entity");
            item.pending_state = Some(next.into());
            item.phase = Phase::Release;
            item.phase_started_ms = self.now_ms;
            Ok(())
        }
    }

    pub fn advance_to(&mut self, now: u64) -> Result<(), Error> {
        if now < self.now_ms {
            return Err(Error::TimeReversed);
        }
        self.now_ms = now;
        let ids: Vec<String> = self.entities.keys().cloned().collect();
        for id in ids {
            let (phase, started, pending) = {
                let e = self.entity(&id)?;
                (e.phase, e.phase_started_ms, e.pending_state.clone())
            };
            let (attack_ms, release_ms) = {
                let phases = &self.state_definition(&id)?.phases;
                (
                    phases.attack_ms.unwrap_or(0),
                    phases.release_ms.unwrap_or(0),
                )
            };
            match phase {
                Phase::Attack if now.saturating_sub(started) >= attack_ms => {
                    self.entities.get_mut(&id).expect("validated entity").phase = Phase::Sustain;
                    self.entities
                        .get_mut(&id)
                        .expect("validated entity")
                        .phase_started_ms = started.saturating_add(attack_ms);
                }
                Phase::Release if now.saturating_sub(started) >= release_ms => {
                    if let Some(next) = pending {
                        let at = started.saturating_add(release_ms);
                        self.enter_state(&id, &next, at)?;
                        let attack = self.state_definition(&id)?.phases.attack_ms.unwrap_or(0);
                        if now.saturating_sub(at) >= attack {
                            let e = self.entities.get_mut(&id).expect("validated entity");
                            e.phase = Phase::Sustain;
                            e.phase_started_ms = at.saturating_add(attack);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn visuals(&self, id: &str) -> Result<Properties, Error> {
        let entity = self.entity(id)?;
        let state = self.state_definition(id)?;
        let mut visuals = state.visuals.clone();
        let phase_visuals = match entity.phase {
            Phase::Attack => &state.phases.attack,
            Phase::Sustain => &state.phases.sustain,
            Phase::Release => &state.phases.release,
        };
        visuals.extend(phase_visuals.clone());
        if let Some(name) = &entity.theme {
            let theme = self
                .themes
                .get(name)
                .ok_or_else(|| Error::Missing(name.clone()))?;
            if let Some(overrides) = theme
                .components
                .get(&entity.definition)
                .and_then(|states| states.get(&entity.state))
            {
                visuals.extend(overrides.clone());
            }
        }
        let elapsed = self.now_ms.saturating_sub(entity.phase_started_ms);
        let pos = match entity.phase {
            Phase::Sustain => (elapsed % 1000) as u16,
            Phase::Attack | Phase::Release => {
                let duration = if entity.phase == Phase::Attack {
                    state.phases.attack_ms.unwrap_or(0)
                } else {
                    state.phases.release_ms.unwrap_or(0)
                };
                if duration == 0 {
                    1000
                } else {
                    ((elapsed.min(duration) as u128) * 1000 / duration as u128) as u16
                }
            }
        };
        let mut resolved: Properties = visuals
            .into_iter()
            .map(|(key, value)| (key, value.at(pos)))
            .collect();
        resolved.extend(entity.overrides.clone());
        Ok(resolved)
    }

    pub fn snapshots(&self) -> Result<Vec<Snapshot>, Error> {
        self.entities
            .iter()
            .map(|(id, entity)| {
                Ok(Snapshot {
                    id: id.clone(),
                    path: self.path(id)?,
                    definition: entity.definition.clone(),
                    state: entity.state.clone(),
                    pending_state: entity.pending_state.clone(),
                    phase: entity.phase,
                    phase_elapsed_ms: self.now_ms.saturating_sub(entity.phase_started_ms),
                    lifecycle: entity.lifecycle,
                    visuals: self.visuals(id)?,
                    properties: entity.properties.clone(),
                    data: entity.data.clone(),
                    context: self.context(id)?,
                })
            })
            .collect()
    }

    pub fn register_handler(
        &mut self,
        component: &str,
        function: &str,
        handler: impl FnMut(&Data, &mut ActionContext) -> Result<Data, Error> + 'static,
    ) -> Result<(), Error> {
        self.entity(component)?;
        nonempty(function)?;
        let key = (component.into(), function.into());
        if self.handlers.contains_key(&key) {
            return Err(Error::Duplicate(format!("{component}.{function}")));
        }
        self.handlers.insert(key, Box::new(handler));
        Ok(())
    }
    pub fn register_function(
        &mut self,
        name: &str,
        handler: impl FnMut(&Data, &mut ActionContext) -> Result<Data, Error> + 'static,
    ) -> Result<(), Error> {
        nonempty(name)?;
        if self.functions.contains_key(name) {
            return Err(Error::Duplicate(name.into()));
        }
        self.functions.insert(name.into(), Box::new(handler));
        Ok(())
    }
    pub fn subscribe(
        &mut self,
        component: &str,
        event: &str,
        listener: impl FnMut(&EventRecord, &mut ActionContext) -> Result<(), Error> + 'static,
    ) -> Result<u64, Error> {
        self.entity(component)?;
        nonempty(event)?;
        self.next_listener = self
            .next_listener
            .checked_add(1)
            .ok_or(Error::LimitExceeded)?;
        self.listeners.insert(
            self.next_listener,
            ((component.into(), event.into()), Box::new(listener)),
        );
        Ok(self.next_listener)
    }
    pub fn unsubscribe(&mut self, token: u64) -> bool {
        self.listeners.remove(&token).is_some()
    }

    pub fn register_flow(&mut self, flow: Flow) -> Result<(), Error> {
        nonempty(&flow.id)?;
        if self.flows.contains_key(&flow.id) {
            return Err(Error::Duplicate(flow.id));
        }
        self.flows.insert(flow.id.clone(), flow);
        Ok(())
    }
    pub fn bind_flow(&mut self, component: &str, event: &str, flow: &str) -> Result<(), Error> {
        self.entity(component)?;
        nonempty(event)?;
        if !self.flows.contains_key(flow) {
            return Err(Error::Missing(flow.into()));
        }
        self.flow_bindings
            .entry((component.into(), event.into()))
            .or_default()
            .push(flow.into());
        Ok(())
    }
    fn apply_actions(
        &mut self,
        actions: Vec<Action>,
        budget: &mut usize,
        stack: &mut BTreeSet<String>,
    ) -> Result<(), Error> {
        for action in actions {
            match action {
                Action::SetState { component, state } => self.set_state(&component, &state)?,
                Action::SetVisual {
                    component,
                    key,
                    value,
                } => self.set_visual(&component, &key, value)?,
                Action::SetData {
                    component,
                    key,
                    value,
                } => self.set_data(&component, &key, value)?,
                Action::Emit {
                    component,
                    event,
                    payload,
                } => self.queue.push_back((component, event, payload)),
                Action::Flow(name) => self.run_flow_inner(&name, stack, budget)?,
            }
        }
        Ok(())
    }
    fn call_registered(
        &mut self,
        component: &str,
        name: &str,
        payload: &Data,
        budget: &mut usize,
        stack: &mut BTreeSet<String>,
    ) -> Result<Data, Error> {
        let state = self.entity(component)?.state.clone();
        let key = (component.into(), name.into());
        let is_local = self.handlers.contains_key(&key);
        let mut handler = if is_local {
            self.handlers.remove(&key)
        } else {
            self.functions.remove(name)
        }
        .ok_or_else(|| Error::Missing(format!("function {component}.{name}")))?;
        let mut ctx = ActionContext::new(component, &state);
        let result = handler(payload, &mut ctx);
        if is_local {
            self.handlers.insert(key, handler);
        } else {
            self.functions.insert(name.into(), handler);
        }
        let value = result?;
        self.apply_actions(ctx.actions, budget, stack)?;
        Ok(value)
    }
    fn run_steps(
        &mut self,
        steps: &[FlowStep],
        stack: &mut BTreeSet<String>,
        budget: &mut usize,
    ) -> Result<(), Error> {
        for step in steps {
            if *budget == 0 {
                return Err(Error::LimitExceeded);
            }
            *budget -= 1;
            match step {
                FlowStep::SetState { component, state } => self.set_state(component, state)?,
                FlowStep::SetVisual {
                    component,
                    key,
                    value,
                } => self.set_visual(component, key, value.clone())?,
                FlowStep::SetData {
                    component,
                    key,
                    value,
                } => self.set_data(component, key, value.clone())?,
                FlowStep::Emit {
                    component,
                    event,
                    payload,
                } => self
                    .queue
                    .push_back((component.clone(), event.clone(), payload.clone())),
                FlowStep::Call {
                    component,
                    function,
                    payload,
                } => {
                    self.call_registered(component, function, payload, budget, stack)?;
                }
                FlowStep::Flow(name) => self.run_flow_inner(name, stack, budget)?,
                FlowStep::IfEqual {
                    component,
                    key,
                    value,
                    then_steps,
                    else_steps,
                } => {
                    let matches = self.entity(component)?.data.get(key) == Some(value);
                    self.run_steps(if matches { then_steps } else { else_steps }, stack, budget)?;
                }
            }
        }
        Ok(())
    }
    fn run_flow_inner(
        &mut self,
        name: &str,
        stack: &mut BTreeSet<String>,
        budget: &mut usize,
    ) -> Result<(), Error> {
        let steps = self
            .flows
            .get(name)
            .ok_or_else(|| Error::Missing(name.into()))?
            .steps
            .clone();
        if !stack.insert(name.into()) {
            return Err(Error::Cycle(format!("flow {name}")));
        }
        let result = self.run_steps(&steps, stack, budget);
        stack.remove(name);
        result
    }
    pub fn run_flow(&mut self, name: &str) -> Result<(), Error> {
        let result = self.run_flow_inner(name, &mut BTreeSet::new(), &mut 1024);
        result?;
        self.drain()
    }

    pub fn emit(&mut self, component: &str, event: &str, payload: Data) -> Result<(), Error> {
        self.queue
            .push_back((component.into(), event.into(), payload));
        self.drain()
    }
    pub fn emit_at_path(
        &mut self,
        path: &[String],
        event: &str,
        payload: Data,
    ) -> Result<(), Error> {
        self.check_path(path)?;
        self.emit(path.last().expect("checked path"), event, payload)
    }
    fn drain(&mut self) -> Result<(), Error> {
        if self.dispatching {
            return Ok(());
        }
        self.dispatching = true;
        let result = (|| {
            let mut remaining = 1024;
            while let Some((component, event, payload)) = self.queue.pop_front() {
                if remaining == 0 {
                    return Err(Error::LimitExceeded);
                }
                remaining -= 1;
                self.dispatch_one(&component, &event, payload, &mut remaining)?;
            }
            Ok(())
        })();
        self.dispatching = false;
        if result.is_err() {
            self.queue.clear();
        }
        result
    }
    fn dispatch_one(
        &mut self,
        component: &str,
        event: &str,
        payload: Data,
        budget: &mut usize,
    ) -> Result<(), Error> {
        let state = self.entity(component)?.state.clone();
        let definition = self
            .state_definition(component)?
            .events
            .get(event)
            .ok_or_else(|| Error::Unavailable(format!("{component}.{event}")))?
            .clone();
        if !definition.payload.accepts(&payload) {
            return Err(Error::WrongPayload(format!("{component}.{event}")));
        }
        let key = (component.into(), definition.function_id.clone());
        let mut handler = self.handlers.remove(&key).ok_or_else(|| {
            Error::Missing(format!("handler {component}.{}", definition.function_id))
        })?;
        let mut ctx = ActionContext::new(component, &state);
        let result = handler(&payload, &mut ctx);
        self.handlers.insert(key, handler);
        let value = result?;
        self.apply_actions(ctx.actions, budget, &mut BTreeSet::new())?;
        let sequence = self.history.len() as u64 + 1;
        let record = EventRecord {
            sequence,
            component: component.into(),
            path: self.path(component)?,
            state,
            event: event.into(),
            function: definition.function_id,
            payload,
            result: value,
        };
        self.history.push(record.clone());
        let tokens: Vec<u64> = self
            .listeners
            .iter()
            .filter_map(|(token, (key, _))| {
                if key.0 == component && key.1 == event {
                    Some(*token)
                } else {
                    None
                }
            })
            .collect();
        for token in tokens {
            if let Some((key, mut listener)) = self.listeners.remove(&token) {
                let mut ctx = ActionContext::new(component, &record.state);
                let response = listener(&record, &mut ctx);
                self.listeners.insert(token, (key, listener));
                response?;
                self.apply_actions(ctx.actions, budget, &mut BTreeSet::new())?;
            }
        }
        let names = self
            .flow_bindings
            .get(&(component.into(), event.into()))
            .cloned()
            .unwrap_or_default();
        for name in names {
            self.run_flow_inner(&name, &mut BTreeSet::new(), budget)?;
        }
        Ok(())
    }

    pub fn resolve_input(
        &self,
        contract: &InputContract,
        raw: &str,
        context: &Context,
        adapter: Option<&mut dyn ModelAdapter>,
    ) -> Result<ResolvedInput, Error> {
        nonempty(&contract.name)?;
        let (value, provider) = if contract.kind == DataKind::Text
            && (contract.allow_raw_text
                || contract.vocabulary.iter().any(|candidate| candidate == raw))
        {
            (Data::Text(raw.into()), "strict".to_owned())
        } else {
            let host = adapter.ok_or_else(|| Error::Adapter("model adapter required".into()))?;
            (
                host.resolve(raw, contract, context)?,
                "host-model".to_owned(),
            )
        };
        if !contract.kind.accepts(&value) {
            return Err(Error::WrongPayload(contract.name.clone()));
        }
        if !contract.vocabulary.is_empty() {
            let Data::Text(text) = &value else {
                return Err(Error::WrongPayload(contract.name.clone()));
            };
            if !contract.vocabulary.contains(text) {
                return Err(Error::Unavailable("input outside vocabulary".into()));
            }
        }
        Ok(ResolvedInput { value, provider })
    }

    pub fn drive(
        &mut self,
        clock: &dyn Clock,
        source: &mut dyn EventSource,
        renderer: &mut dyn Renderer,
    ) -> Result<(), Error> {
        self.advance_to(clock.now_ms()?)?;
        let mut budget = 1024;
        while let Some((component, event, payload)) = source.next_event()? {
            if budget == 0 {
                return Err(Error::LimitExceeded);
            }
            budget -= 1;
            self.emit(&component, &event, payload)?;
        }
        renderer.render(&self.snapshots()?)
    }
    pub fn persist(&self, namespace: &str, storage: &mut dyn Storage) -> Result<(), Error> {
        nonempty(namespace)?;
        storage.store(namespace, &self.snapshots()?)
    }
    pub fn external(
        &self,
        name: &str,
        payload: &Data,
        host: Option<&mut dyn ExternalIo>,
    ) -> Result<Data, Error> {
        nonempty(name)?;
        host.ok_or_else(|| Error::Adapter("external I/O adapter required".into()))?
            .invoke(name, payload)
    }
}
