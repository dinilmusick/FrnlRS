//! Headless consumer: a native host owns clock, input and rendering.
use frnlrs::semantic::*;
use std::collections::BTreeMap;

struct Console;
impl Renderer for Console {
    fn render(&mut self, snapshots: &[Snapshot]) -> Result<(), Error> {
        let button = snapshots.iter().find(|item| item.id == "submit").ok_or_else(|| Error::Missing("submit".into()))?;
        assert_eq!(button.data.get("submitted"), Some(&Data::Bool(true)));
        println!("FrnlRS semantic consumer: {} state={}, event routed", button.id, button.state);
        Ok(())
    }
}
struct ManualClock;
impl Clock for ManualClock { fn now_ms(&self) -> Result<u64, Error> { Ok(0) } }
struct Click;
impl EventSource for Click {
    fn next_event(&mut self) -> Result<Option<(String, String, Data)>, Error> {
        Ok(Some(("submit".into(), "click".into(), Data::Null))).and_then(|item| {
            // A host event source must consume its events exactly once.
            Ok(item)
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut r = Runtime::new();
    let mut state = StateDefinition::default();
    state.events.insert("click".into(), EventDefinition { function_id: "submit".into(), payload: DataKind::Null });
    r.register_definition(ComponentDefinition {
        name: "button".into(), initial_state: "idle".into(),
        states: BTreeMap::from([("idle".into(), state)]), context: Context::default(),
    })?;
    r.mount("submit", "button", None)?;
    r.register_handler("submit", "submit", |_, ctx| {
        ctx.set_data("submitted", Data::Bool(true));
        Ok(Data::Bool(true))
    })?;
    // Events are supplied by the host; no synthetic UI callbacks exist in the crate.
    struct Single(Option<(String, String, Data)>);
    impl EventSource for Single {
        fn next_event(&mut self) -> Result<Option<(String, String, Data)>, Error> { Ok(self.0.take()) }
    }
    let mut source = Single(Some(("submit".into(), "click".into(), Data::Null)));
    r.drive(&ManualClock, &mut source, &mut Console)?;
    assert_eq!(r.history().len(), 1);
    Ok(())
}
