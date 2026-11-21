# FrnlRS

Portable, headless semantic frontend runtime in Rust, built from pinned FrnlTS contracts.
**No React implementation or built-in renderer.** Native applications and other hosts
supply the window, event source, clock, persistence and optional model inference.

- **Semantic engine**: `frnlrs::semantic::Runtime` — state definitions and transitions,
  state-specific event handlers, deterministic queued events, subscriptions, phase timing,
  interpolated curves, themes, flows, scoped context/data, reusable functions and libraries,
  and stable renderer snapshots.
- **Mounted control surface**: `frnlrs::Surface` — an independent compatibility API for
  live native control availability, hierarchy and real control callbacks, preserved from PR #1.
- **Adapters**: host-defined `Renderer`, `Clock`, `EventSource`, `Storage`,
  `ExternalIo`, and `ModelAdapter` traits. Missing optional adapters return errors,
  not fake successful actions.

Canonical FrnlTS: `a9339b9e92f9ef375ac164e274842644dca60d8a`.
Native foundation reviewed: GutRS `3e4ad1823166b37ddc437aab04277b42c124ce36`.
No direct GutRS binding is required: its optional interfaces remain unverified at this revision.

The [current parity matrix](docs/PARITY.md) and [semantic contract / adapter
reference](docs/SEMANTIC_PARITY.md) distinguish implemented headless functionality
from platform-specific features deliberately left for later integration.

## Verify

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo run --example native_consumer
cargo run --example semantic_consumer
cargo clippy --all-targets -- -D warnings
rustup target add wasm32-wasip1
cargo check --target wasm32-wasip1
```

Both examples run real crate APIs and verify callback / state behavior without opening
a native window. **Neither example certifies a desktop, embedded display, operating-system
UI, browser integration, accessibility adapter or deployed application.** The core currently
requires `std`; `no_std`/`alloc` support and direct GutRS integration remain future work.
