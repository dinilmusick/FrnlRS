# Portable target and external consumer

The FrnlRS API stays renderer-neutral. The public Surface API and the
semantic Runtime use alloc types and core traits. The crate builds with
no_std when the default std feature is disabled.

```sh
cargo check --lib --no-default-features --target thumbv7em-none-eabi
cargo check --target wasm32-wasip1
cargo run --manifest-path examples/external-consumer/Cargo.toml
```

An embedded host must provide a global allocator and any required transport,
clock, storage, events and renderer implementations when linking its final
executable. This library performs no heap setup, file I/O, display output or
model inference itself.

The independent consumer is a standalone Cargo crate with a frnlrs dependency
using default-features = false. It verifies the public API from another crate,
event-source input, real callback dispatch, deterministic rendered state,
storage and the legacy live-control surface. This is a headless integration
test, not a physical GUI or screen-reader test.

No gutrs dependency is forced onto embedded targets. Pinned GutRS commit
3e4ad1823166b37ddc437aab04277b42c124ce36 has standard-library-backed
semantic model adapters. Hosts can bridge those without importing such
dependencies into FrnlRS. Missing optional model and I/O adapters fail
explicitly.
