# FrnlRS

Runtime-neutral Rust semantic frontend and programmatic UI surface for ULTRST applications.

## Upstream and parity scope

- Canonical upstream: [FrnlTS](https://github.com/dinilmusick/FrnlTS), pinned main commit `6c8cc81e7304e478e51e0810160eab2969fb0636`.
- Browser/PHP behavioral reference: [FrnlWP](https://github.com/dinilmusick/FrnlWP), whose UI surface exposes mounted components, ancestor visibility and enabled state, accessible controls and real event paths.
- Initial port: live component registry, hierarchy/cycle enforcement, snapshots, inherited visibility and enabled state, read-only semantics, typed input/click/change dispatch, and application-injected native event callbacks.
- Never spawns agents, synthesizes callbacks or claims an unavailable control executed.

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

The native renderer is supplied by the consuming application. Mount/unmount, availability updates and event handlers must be connected to its **real** UI framework; this crate does not render controls or connect to a browser. Complete FrnlTS parity (DOM geometry, framework subscriptions, focus, screen-reader semantics, layout, event transport and published package adoption) is not claimed. No application integration or native platform smoke test is yet verified.
