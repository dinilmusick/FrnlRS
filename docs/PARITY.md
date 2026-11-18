# FrnlTS → FrnlRS parity inventory

Pinned upstream: `dinilmusick/FrnlTS@6c8cc81e7304e478e51e0810160eab2969fb0636`, npm `@ultrst-armada/frnlts@0.5.26`. The package exports `.`, `./runtime`, `./react`, `./validate`, `./cli`.

The upstream `src/runtime/index.ts` re-exports core, resources, registry, routing, accessibility, dependencies, store, validation, events, libraries, facade, themes, component definition, orchestration, frontend API, measurement, semantic input and UI session. The React entry exports `useFrnlComponent` and semantic input components. FrnlRS does **not** expose React bindings.

| Behavior / upstream boundary | Rust counterpart | Evidence / gap |
| --- | --- | --- |
| Registered component lifecycle | `Surface::mount`, `unmount` | Only registered components inspectable; unmounted children become roots |
| Parent scopes and inheritance | `Surface::set_parent`, `inspect` | Reject nonexistent ancestors/cycles; parent-hidden/disabled suppress descendants |
| Semantic control availability | `Control::{Click,Input,Change}`, `Descriptor` | Read-only, disabled and invisible controls unavailable |
| Real UI event route | `Surface::invoke`, injected application `FnMut` | Handler errors propagate; no synthetic event execution |
| Typed scalar invocation | `Value::{None,Text,Bool}` | Payload mismatch rejected; no arbitrary object payloads |
| Native consumer integration | `examples/native_consumer.rs` | Executed against crate by public CI, not a real GUI |
| Resource graphs, state schemas, themes | Not implemented | Needs idiomatic Rust providers |
| Routing, accessibility roles/focus, geometry | Not implemented | Requires real target-platform window/accessibility adapters |
| Semantic input and UI session authorization | Not implemented | Must use canonical auth and session, not a replacement |
| React hooks and rendering | Not applicable | Concrete rendering belongs to native framework adapters |
| CLI, validation, dependency migrations | Not implemented | Port relevant contracts and tests separately |

## Executable cross-language reference

The `tests/surface_contract.rs` cases follow FrnlWP browser UI-surface behavior for mounted hierarchy, inherited hidden/disabled state, read-only controls, click/input/change payloads, and event invocation. FrnlTS source is the authoritative long-term target; FrnlWP is a behavior fixture, not a replacement.

Verification gates: `cargo fmt --all -- --check`, `cargo test --all-targets`, `cargo run --example native_consumer`, and `cargo clippy --all-targets -- -D warnings`.

**Outstanding:** No native window adapter, actual keyboard/mouse hardware interaction, focus or screen-reader integration, external consumer deployment, or full cross-language release parity is certified.
