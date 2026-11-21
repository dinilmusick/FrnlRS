//! Minimal native application adapter: the app owns its UI event path.
use frnlrs::{Control, Descriptor, Surface, Value};
use std::sync::{Arc, Mutex};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let native_events = Arc::new(Mutex::new(Vec::<(Control, Value)>::new()));
    let destination = Arc::clone(&native_events);
    let mut ui = Surface::default();
    ui.mount(
        Descriptor::new("native.submit", [Control::Click]),
        move |control, value| {
            destination
                .lock()
                .expect("event lock")
                .push((control, value.clone()));
            Ok(())
        },
    )?;
    let live = ui.inspect()?;
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].controls, vec![Control::Click]);
    ui.invoke("native.submit", Control::Click, Value::None)?;
    assert_eq!(
        native_events.lock().expect("event lock").as_slice(),
        &[(Control::Click, Value::None)]
    );
    ui.unmount("native.submit")?;
    assert!(ui.inspect()?.is_empty());
    println!("FrnlRS native consumer adapter: OK");
    Ok(())
}
