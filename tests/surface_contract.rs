use frnlrs::{Control, Descriptor, Surface, SurfaceError, Value};
use std::sync::{Arc, Mutex};

fn mount(surface: &mut Surface, id: &str, parent: Option<&str>, controls: &[Control]) {
    let mut descriptor = Descriptor::new(id, controls.iter().copied());
    descriptor.parent_id = parent.map(str::to_owned);
    surface
        .mount(descriptor, |_control, _value| Ok(()))
        .unwrap();
}

#[test]
fn mounted_hierarchy_is_live_and_cycle_safe() {
    let mut surface = Surface::default();
    mount(&mut surface, "root", None, &[Control::Click]);
    mount(&mut surface, "child", Some("root"), &[Control::Input]);
    mount(&mut surface, "leaf", Some("child"), &[Control::Change]);
    assert_eq!(surface.inspect().unwrap().len(), 3);
    assert_eq!(
        surface.set_parent("root", Some("leaf")),
        Err(SurfaceError::Cycle("root".to_owned()))
    );
    assert_eq!(
        surface.set_parent("leaf", Some("missing")),
        Err(SurfaceError::Unknown("missing".to_owned()))
    );
    assert_eq!(
        surface.set_parent("root", Some("root")),
        Err(SurfaceError::Cycle("root".to_owned()))
    );
    assert_eq!(
        surface.inspect().unwrap()[0].parent_id,
        Some("root".to_owned())
    );
    surface.unmount("root").unwrap();
    assert_eq!(surface.inspect().unwrap().len(), 2);
    let child = surface
        .inspect()
        .unwrap()
        .into_iter()
        .find(|component| component.id == "child")
        .unwrap();
    assert_eq!(child.parent_id, None);
    surface.set_parent("leaf", None).unwrap();
    assert_eq!(
        surface
            .inspect()
            .unwrap()
            .into_iter()
            .find(|component| component.id == "leaf")
            .unwrap()
            .parent_id,
        None
    );
    assert_eq!(
        surface.unmount("unknown"),
        Err(SurfaceError::Unknown("unknown".to_owned()))
    );
}

#[test]
fn disabled_hidden_readonly_controls_fail_closed() {
    let mut surface = Surface::default();
    mount(&mut surface, "root", None, &[Control::Click]);
    mount(
        &mut surface,
        "field",
        Some("root"),
        &[Control::Input, Control::Change],
    );
    surface
        .update_availability("root", false, true, false)
        .unwrap();
    assert_eq!(
        surface.invoke("field", Control::Input, Value::Text("edited".to_owned())),
        Err(SurfaceError::Unavailable("field".to_owned()))
    );
    surface
        .update_availability("root", true, false, false)
        .unwrap();
    assert!(
        !surface
        .inspect()
        .unwrap()
        .into_iter()
        .find(|item| item.id == "field")
        .unwrap()
        .enabled);
    surface
        .update_availability("root", true, true, false)
        .unwrap();
    surface
        .update_availability("field", true, true, true)
        .unwrap();
    assert_eq!(
        surface.invoke("field", Control::Change, Value::Bool(true)),
        Err(SurfaceError::Unavailable("field".to_owned()))
    );
    assert!(
        surface
            .inspect()
            .unwrap()
            .into_iter()
            .find(|item| item.id == "field")
            .unwrap()
            .controls
            .is_empty()
    );
    surface
        .update_availability("field", true, true, false)
        .unwrap();
    assert!(surface
        .invoke("field", Control::Change, Value::Bool(true))
        .is_ok());
    assert_eq!(
        surface.invoke("field", Control::Input, Value::None),
        Err(SurfaceError::InvalidPayload)
    );
}

#[test]
fn invocation_calls_real_application_handler_and_propagates_errors() {
    let calls = Arc::new(Mutex::new(Vec::<(Control, Value)>::new()));
    let shared = Arc::clone(&calls);
    let mut surface = Surface::default();
    surface
        .mount(
            Descriptor::new("submit", [Control::Click, Control::Change]),
            move |control, value| {
                shared.lock().unwrap().push((control, value.clone()));
                if value == &Value::Text("reject".to_owned()) {
                    return Err(SurfaceError::Handler("rejected".to_owned()));
                }
                Ok(())
            },
        )
        .unwrap();
    surface
        .invoke("submit", Control::Click, Value::None)
        .unwrap();
    surface
        .invoke(
            "submit",
            Control::Change,
            Value::Text("accepted".to_owned()),
        )
        .unwrap();
    assert_eq!(
        surface.invoke("submit", Control::Change, Value::Text("reject".to_owned())),
        Err(SurfaceError::Handler("rejected".to_owned()))
    );
    assert_eq!(
        surface.invoke("submit", Control::Input, Value::Text("ignored".to_owned())),
        Err(SurfaceError::Unavailable("submit".to_owned()))
    );
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[
            (Control::Click, Value::None),
            (Control::Change, Value::Text("accepted".to_owned())),
            (Control::Change, Value::Text("reject".to_owned())),
        ]
    );
    assert_eq!(
        surface.invoke("missing", Control::Click, Value::None),
        Err(SurfaceError::Unknown("missing".to_owned()))
    );
    surface.unmount("submit").unwrap();
    assert!(surface.inspect().unwrap().is_empty());
}

#[test]
fn duplicate_and_invalid_mounts_are_rejected() {
    let mut surface = Surface::default();
    assert!(matches!(
        surface.mount(Descriptor::new(" ", [Control::Click]), |_, _| Ok(())),
        Err(SurfaceError::EmptyId)
    ));
    assert_eq!(
        surface.mount(
            Descriptor {
                parent_id: Some("absent".to_owned()),
                ..Descriptor::new("child", [Control::Click])
            },
            |_, _| Ok(())
        ),
        Err(SurfaceError::Unknown("absent".to_owned()))
    );
    mount(&mut surface, "same", None, &[Control::Click]);
    assert_eq!(
        surface.mount(Descriptor::new("same", [Control::Click]), |_, _| Ok(())),
        Err(SurfaceError::Duplicate("same".to_owned()))
    );
    assert_eq!(surface.inspect().unwrap().len(), 1);
}
