//! Separate downstream Cargo crate: tests FrnlRS with std feature disabled.
//! The consuming application owns all platform event, clock, render and storage adapters.

use frnlrs::semantic::{
    Clock, ComponentDefinition, Context, Data, DataKind, Error, EventDefinition, EventSource,
    Renderer, Runtime, Snapshot, StateDefinition, Storage,
};
use frnlrs::{Control, Descriptor, Surface, Value};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

struct ManualClock;
impl Clock for ManualClock {
    fn now_ms(&self) -> Result<u64, Error> {
        Ok(250)
    }
}

struct InputQueue(VecDeque<(String, String, Data)>);
impl EventSource for InputQueue {
    fn next_event(&mut self) -> Result<Option<(String, String, Data)>, Error> {
        Ok(self.0.pop_front())
    }
}

#[derive(Default)]
struct Host {
    rendered: Vec<Snapshot>,
    stored: Vec<Snapshot>,
}
impl Renderer for Host {
    fn render(&mut self, snapshots: &[Snapshot]) -> Result<(), Error> {
        self.rendered = snapshots.to_vec();
        Ok(())
    }
}
impl Storage for Host {
    fn store(&mut self, namespace: &str, snapshots: &[Snapshot]) -> Result<(), Error> {
        assert_eq!(namespace, "workspace");
        self.stored = snapshots.to_vec();
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = Runtime::new();
    let mut ready = StateDefinition::default();
    ready.events.insert(
        "submit".into(),
        EventDefinition {
            function_id: "submit_form".into(),
            payload: DataKind::Text,
        },
    );
    runtime.register_definition(ComponentDefinition {
        name: "NativeForm".into(),
        initial_state: "ready".into(),
        states: BTreeMap::from([("ready".into(), ready)]),
        context: Context::default(),
    })?;
    runtime.mount("form", "NativeForm", None)?;
    runtime.register_handler("form", "submit_form", |payload, ctx| {
        ctx.set_data("last_submission", payload.clone());
        Ok(Data::Bool(true))
    })?;

    let mut source = InputQueue(VecDeque::from([(
        "form".into(),
        "submit".into(),
        Data::Text("validated".into()),
    )]));
    let mut host = Host::default();
    runtime.drive(&ManualClock, &mut source, &mut host)?;
    assert_eq!(host.rendered.len(), 1);
    assert_eq!(runtime.history().len(), 1);
    assert_eq!(runtime.history()[0].function, "submit_form");
    assert_eq!(
        host.rendered[0].data.get("last_submission"),
        Some(&Data::Text("validated".into()))
    );
    runtime.persist("workspace", &mut host)?;
    assert_eq!(host.stored, host.rendered);

    let calls = Arc::new(Mutex::new(0_u32));
    let counter = Arc::clone(&calls);
    let mut surface = Surface::default();
    surface.mount(Descriptor::new("submit", [Control::Click]), move |_, payload| {
        assert_eq!(payload, &Value::None);
        *counter.lock().expect("counter lock poisoned") += 1;
        Ok(())
    })?;
    surface.invoke("submit", Control::Click, Value::None)?;
    assert_eq!(*calls.lock().expect("counter lock poisoned"), 1);
    println!("FrnlRS external no-std-feature consumer: PASS");
    Ok(())
}
