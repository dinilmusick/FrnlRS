use frnlrs::semantic::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

fn definition() -> ComponentDefinition {
    let mut idle = StateDefinition::default();
    idle.events.insert(
        "activate".into(),
        EventDefinition {
            function_id: "open".into(),
            payload: DataKind::Integer,
        },
    );
    idle.events.insert(
        "follow".into(),
        EventDefinition {
            function_id: "follow".into(),
            payload: DataKind::Null,
        },
    );
    idle.phases.attack_ms = Some(100);
    idle.phases.release_ms = Some(20);
    idle.visuals.insert(
        "opacity".into(),
        Visual::Curve(vec![
            Keyframe { at_ms: 0, value: 0 },
            Keyframe {
                at_ms: 1000,
                value: 100,
            },
        ]),
    );
    let mut active = StateDefinition::default();
    active.events.insert(
        "activate".into(),
        EventDefinition {
            function_id: "close".into(),
            payload: DataKind::Integer,
        },
    );
    active.phases.attack_ms = Some(10);
    active
        .visuals
        .insert("opacity".into(), Visual::Fixed(Data::Integer(50)));
    ComponentDefinition {
        name: "Button".into(),
        initial_state: "idle".into(),
        states: BTreeMap::from([("idle".into(), idle), ("active".into(), active)]),
        context: Context {
            application: Some("demo".into()),
            ..Context::default()
        },
    }
}

fn runtime() -> Runtime {
    let mut r = Runtime::new();
    r.register_definition(definition()).unwrap();
    r.mount("parent", "Button", None).unwrap();
    r.mount("child", "Button", Some("parent")).unwrap();
    r
}

#[test]
fn pinned_lifecycle_and_scope_contract() {
    assert!(LifecyclePhase::Created.can_transition(LifecyclePhase::Initializing));
    assert!(!LifecyclePhase::Created.can_transition(LifecyclePhase::Active));
    let mut r = runtime();
    assert_eq!(r.path("child").unwrap(), vec!["parent", "child"]);
    r.check_path(&["parent".into(), "child".into()]).unwrap();
    assert!(r.check_path(&["child".into()]).is_err());
    assert!(matches!(
        r.set_parent("parent", Some("child")),
        Err(Error::Cycle(_))
    ));
    assert!(matches!(
        r.set_parent("child", Some("missing")),
        Err(Error::Missing(_))
    ));
    r.transition_lifecycle("child", LifecyclePhase::Initializing)
        .unwrap();
    assert!(r
        .transition_lifecycle("child", LifecyclePhase::Active)
        .is_err());
    r.transition_lifecycle("child", LifecyclePhase::Mounted)
        .unwrap();
    r.transition_lifecycle("child", LifecyclePhase::Active)
        .unwrap();
    r.unmount("parent").unwrap();
    assert_eq!(r.path("child").unwrap(), vec!["child"]);
}

#[test]
fn current_state_handler_reentrant_fifo_and_unsubscribe() {
    let mut r = runtime();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let opened = Rc::clone(&calls);
    r.register_handler("child", "open", move |_, ctx| {
        opened.borrow_mut().push("open");
        ctx.emit("follow", Data::Null);
        ctx.set_state("active");
        Ok(Data::Text("opened".into()))
    })
    .unwrap();
    let followed = Rc::clone(&calls);
    r.register_handler("child", "follow", move |_, _| {
        followed.borrow_mut().push("follow");
        Ok(Data::Null)
    })
    .unwrap();
    let closed = Rc::clone(&calls);
    r.register_handler("child", "close", move |_, _| {
        closed.borrow_mut().push("close");
        Ok(Data::Null)
    })
    .unwrap();
    let listener = Rc::clone(&calls);
    let subscription = r
        .subscribe("child", "activate", move |record, _| {
            assert_eq!(record.state, "idle");
            assert_eq!(record.function, "open");
            listener.borrow_mut().push("listener");
            Ok(())
        })
        .unwrap();
    r.emit("child", "activate", Data::Integer(1)).unwrap();
    assert_eq!(&*calls.borrow(), &["open", "listener", "follow"]);
    assert_eq!(r.history()[0].result, Data::Text("opened".into()));
    assert_eq!(r.history()[1].event, "follow");
    assert_eq!(
        r.entity("child").unwrap().pending_state,
        Some("active".into())
    );
    r.advance_to(30).unwrap();
    assert_eq!(r.entity("child").unwrap().state, "active");
    assert_eq!(r.history()[0].path, vec!["parent", "child"]);
    assert!(r.unsubscribe(subscription));
    r.emit("child", "activate", Data::Integer(2)).unwrap();
    assert_eq!(calls.borrow().last(), Some(&"close"));
    assert_eq!(r.history()[2].function, "close");
}

#[test]
fn attack_sustain_release_and_theme_precedence() {
    let mut r = runtime();
    assert_eq!(r.entity("child").unwrap().phase, Phase::Attack);
    r.advance_to(50).unwrap();
    assert_eq!(r.visuals("child").unwrap()["opacity"], Data::Integer(50));
    r.advance_to(100).unwrap();
    assert_eq!(r.entity("child").unwrap().phase, Phase::Sustain);
    assert_eq!(r.visuals("child").unwrap()["opacity"], Data::Integer(0));
    r.advance_to(1350).unwrap();
    assert_eq!(r.visuals("child").unwrap()["opacity"], Data::Integer(25));
    let theme = Theme {
        components: BTreeMap::from([(
            "Button".into(),
            BTreeMap::from([(
                "idle".into(),
                BTreeMap::from([("opacity".into(), Visual::Fixed(Data::Integer(77)))]),
            )]),
        )]),
    };
    r.register_theme("dark", theme).unwrap();
    r.apply_theme("child", "dark").unwrap();
    assert_eq!(r.visuals("child").unwrap()["opacity"], Data::Integer(77));
    r.set_visual("child", "opacity", Data::Integer(92)).unwrap();
    assert_eq!(r.visuals("child").unwrap()["opacity"], Data::Integer(92));
    r.apply_theme("child", "dark").unwrap();
    assert_eq!(r.visuals("child").unwrap()["opacity"], Data::Integer(77));
    r.set_state("child", "active").unwrap();
    assert_eq!(r.entity("child").unwrap().phase, Phase::Release);
    r.advance_to(1370).unwrap();
    assert_eq!(r.entity("child").unwrap().phase, Phase::Attack);
    r.advance_to(1380).unwrap();
    assert_eq!(r.entity("child").unwrap().phase, Phase::Sustain);
    assert_eq!(r.visuals("child").unwrap()["opacity"], Data::Integer(50));
    assert_eq!(r.state_changes().len(), 1);
    assert_eq!(r.state_changes()[0].from, "idle");
    assert!(matches!(r.advance_to(1), Err(Error::TimeReversed)));
}

#[test]
fn flows_branch_data_and_cycle_protection() {
    let mut r = runtime();
    r.register_handler("child", "open", |_, _| Ok(Data::Null))
        .unwrap();
    r.register_flow(Flow {
        id: "entry".into(),
        steps: vec![
            FlowStep::SetData {
                component: "child".into(),
                key: "ready".into(),
                value: Data::Bool(true),
            },
            FlowStep::IfEqual {
                component: "child".into(),
                key: "ready".into(),
                value: Data::Bool(true),
                then_steps: vec![FlowStep::SetVisual {
                    component: "child".into(),
                    key: "scale".into(),
                    value: Data::Integer(2),
                }],
                else_steps: vec![FlowStep::SetState {
                    component: "child".into(),
                    state: "active".into(),
                }],
            },
        ],
    })
    .unwrap();
    r.bind_flow("child", "activate", "entry").unwrap();
    r.emit("child", "activate", Data::Integer(1)).unwrap();
    assert_eq!(r.entity("child").unwrap().data["ready"], Data::Bool(true));
    assert_eq!(r.visuals("child").unwrap()["scale"], Data::Integer(2));
    r.register_flow(Flow {
        id: "recursive".into(),
        steps: vec![FlowStep::Flow("recursive".into())],
    })
    .unwrap();
    assert!(matches!(r.run_flow("recursive"), Err(Error::Cycle(_))));
}

#[test]
fn context_isolation_libraries_and_invalid_definitions() {
    let mut r = runtime();
    let parent = Context {
        workspace: Some("workspace-A".into()),
        fields: BTreeMap::from([("shared".into(), Data::Bool(true))]),
        ..Context::default()
    };
    r.set_context("parent", parent).unwrap();
    r.set_context(
        "child",
        Context {
            page: Some("landing".into()),
            ..Context::default()
        },
    )
    .unwrap();
    assert_eq!(
        r.context("child").unwrap().workspace.as_deref(),
        Some("workspace-A")
    );
    assert_eq!(r.context("child").unwrap().page.as_deref(), Some("landing"));
    assert_eq!(r.context("parent").unwrap().page, None);
    r.set_page_data("landing", "title", Data::Text("hello".into()))
        .unwrap();
    assert_eq!(
        r.page_data("landing").unwrap()["title"],
        Data::Text("hello".into())
    );
    r.register_library("app.utility", Data::Bool(true)).unwrap();
    assert_eq!(r.library("app.utility").unwrap(), &Data::Bool(true));
    assert!(r.register_library("app.utility", Data::Null).is_err());
    let mut invalid = definition();
    invalid.initial_state = "unknown".into();
    assert!(matches!(
        r.register_definition(invalid),
        Err(Error::Invalid(_))
    ));
    assert!(r
        .register_theme(
            "invalid",
            Theme {
                components: BTreeMap::from([(
                    "Button".into(),
                    BTreeMap::from([("nope".into(), BTreeMap::new()),])
                )]),
            }
        )
        .is_err());
}

#[test]
fn strict_inputs_require_explicit_host_model_and_validate_output() {
    struct Model;
    impl ModelAdapter for Model {
        fn resolve(&mut self, _: &str, _: &InputContract, _: &Context) -> Result<Data, Error> {
            Ok(Data::Integer(42))
        }
    }
    let r = runtime();
    let contract = InputContract {
        name: "answer".into(),
        kind: DataKind::Integer,
        vocabulary: vec![],
        allow_raw_text: false,
    };
    assert!(matches!(
        r.resolve_input(&contract, "forty-two", &Context::default(), None),
        Err(Error::Adapter(_))
    ));
    let mut model = Model;
    assert_eq!(
        r.resolve_input(
            &contract,
            "forty-two",
            &Context::default(),
            Some(&mut model)
        )
        .unwrap()
        .value,
        Data::Integer(42)
    );
    let text = InputContract {
        name: "choice".into(),
        kind: DataKind::Text,
        vocabulary: vec!["yes".into()],
        allow_raw_text: true,
    };
    assert!(r
        .resolve_input(&text, "unknown", &Context::default(), None)
        .is_err());
    assert_eq!(
        r.resolve_input(&text, "yes", &Context::default(), None)
            .unwrap()
            .provider,
        "strict"
    );
}

#[test]
fn typed_host_renderer_event_source_clock_and_storage() {
    struct FixedClock;
    impl Clock for FixedClock {
        fn now_ms(&self) -> Result<u64, Error> {
            Ok(120)
        }
    }
    struct Events(VecDeque<(String, String, Data)>);
    impl EventSource for Events {
        fn next_event(&mut self) -> Result<Option<(String, String, Data)>, Error> {
            Ok(self.0.pop_front())
        }
    }
    #[derive(Default)]
    struct Sink {
        last: Vec<Snapshot>,
    }
    impl Renderer for Sink {
        fn render(&mut self, snapshots: &[Snapshot]) -> Result<(), Error> {
            self.last = snapshots.to_vec();
            Ok(())
        }
    }
    impl Storage for Sink {
        fn store(&mut self, _: &str, snapshots: &[Snapshot]) -> Result<(), Error> {
            self.last = snapshots.to_vec();
            Ok(())
        }
    }
    struct Io;
    impl ExternalIo for Io {
        fn invoke(&mut self, _: &str, payload: &Data) -> Result<Data, Error> {
            Ok(payload.clone())
        }
    }
    let mut r = runtime();
    r.register_handler("child", "open", |_, _| Ok(Data::Integer(7)))
        .unwrap();
    let mut source = Events(VecDeque::from([(
        "child".into(),
        "activate".into(),
        Data::Integer(3),
    )]));
    let mut sink = Sink::default();
    r.drive(&FixedClock, &mut source, &mut sink).unwrap();
    assert_eq!(sink.last.len(), 2);
    assert_eq!(r.history().len(), 1);
    r.persist("ui", &mut sink).unwrap();
    assert!(matches!(
        r.external("call", &Data::Null, None),
        Err(Error::Adapter(_))
    ));
    assert_eq!(
        r.external("call", &Data::Integer(9), Some(&mut Io))
            .unwrap(),
        Data::Integer(9)
    );
}

#[test]
fn handler_failure_payload_guards_and_reentrant_budget() {
    let mut r = runtime();
    assert!(matches!(
        r.emit("child", "activate", Data::Text("bad".into())),
        Err(Error::WrongPayload(_))
    ));
    assert!(matches!(
        r.emit("child", "activate", Data::Integer(1)),
        Err(Error::Missing(_))
    ));
    r.register_handler("child", "open", |_, _| {
        Err(Error::Handler("failure".into()))
    })
    .unwrap();
    assert!(matches!(
        r.emit("child", "activate", Data::Integer(1)),
        Err(Error::Handler(_))
    ));
    assert!(r.history().is_empty());

    let mut loop_engine = Runtime::new();
    let mut state = StateDefinition::default();
    state.events.insert(
        "loop".into(),
        EventDefinition {
            function_id: "again".into(),
            payload: DataKind::Null,
        },
    );
    loop_engine
        .register_definition(ComponentDefinition {
            name: "loop".into(),
            initial_state: "idle".into(),
            states: BTreeMap::from([("idle".into(), state)]),
            context: Context::default(),
        })
        .unwrap();
    loop_engine.mount("loop", "loop", None).unwrap();
    loop_engine
        .register_handler("loop", "again", |_, ctx| {
            ctx.emit("loop", Data::Null);
            Ok(Data::Null)
        })
        .unwrap();
    assert!(matches!(
        loop_engine.emit("loop", "loop", Data::Null),
        Err(Error::LimitExceeded)
    ));
    assert_eq!(loop_engine.history().len(), 1024);
    assert!(loop_engine.emit("loop", "not-defined", Data::Null).is_err());
}

#[test]
fn pinned_cross_language_fixture_phase_contract() {
    let fixture = include_str!("../fixtures/frnlts-phases.tsv");
    let mut r = runtime();
    for line in fixture
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let parts: Vec<_> = line.split('\t').collect();
        let at: u64 = parts[0].parse().unwrap();
        r.advance_to(at).unwrap();
        let actual = format!("{:?}", r.entity("child").unwrap().phase);
        assert_eq!(actual, parts[1], "fixture mismatch at timestamp {at}");
    }
}

#[test]
fn semantic_function_registry_executes_and_rejects_duplicates_without_overwriting() {
    let mut r = runtime();
    r.register_handler("child", "open", |_, _| Ok(Data::Integer(12)))
        .unwrap();
    assert!(matches!(
        r.register_handler("child", "open", |_, _| Ok(Data::Integer(99))),
        Err(Error::Duplicate(_))
    ));
    r.emit("child", "activate", Data::Integer(1)).unwrap();
    assert_eq!(r.history()[0].result, Data::Integer(12));

    r.register_function("apply", |payload, ctx| {
        ctx.set_data("value", payload.clone());
        Ok(payload.clone())
    })
    .unwrap();
    assert!(matches!(
        r.register_function("apply", |_, _| Ok(Data::Null)),
        Err(Error::Duplicate(_))
    ));
    r.register_flow(Flow {
        id: "do-apply".into(),
        steps: vec![FlowStep::Call {
            component: "child".into(),
            function: "apply".into(),
            payload: Data::Integer(42),
        }],
    })
    .unwrap();
    r.run_flow("do-apply").unwrap();
    assert_eq!(r.entity("child").unwrap().data["value"], Data::Integer(42));

    r.register_function("recursive-call", |_, ctx| {
        ctx.flow("indirect-cycle");
        Ok(Data::Null)
    })
    .unwrap();
    r.register_flow(Flow {
        id: "indirect-cycle".into(),
        steps: vec![FlowStep::Call {
            component: "child".into(),
            function: "recursive-call".into(),
            payload: Data::Null,
        }],
    })
    .unwrap();
    assert!(matches!(r.run_flow("indirect-cycle"), Err(Error::Cycle(_))));
}
