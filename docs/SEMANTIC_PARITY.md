# Wave 2 semantic parity contract

Canonical: FrnlTS at a9339b9e92f9ef375ac164e274842644dca60d8a.
Native utility reference: GutRS at 3e4ad1823166b37ddc437aab04277b42c124ce36.
This crate is std-only and uses no unstable GutRS bindings. Rust renderers, embedded
targets, and bare-metal adapters may use the public semantic types without React.
The pre-existing Surface remains available for actual mounted native control gating.

| FrnlTS contract | Rust contract | Executable evidence |
| --- | --- | --- |
| component-definition.ts state/event/visual schemas | ComponentDefinition, StateDefinition, EventDefinition, Visual | semantic_contract and invalid-definition tests |
| events.ts state-specific function ID lookup | Runtime::emit, register_handler | handler resolution and FIFO tests |
| events.ts component scope path | Runtime::path, check_path | nested path and cycle tests |
| orchestration.ts subscriptions | Runtime::subscribe, unsubscribe, bind_flow | unsubscribe and flow tests |
| store.ts semantic entities/data/libraries | Runtime::mount, entity, set_data, register_library | isolation and data tests |
| themes.ts precedence | state -> phase -> theme -> runtime overrides | phase/theme test |
| phases.md Attack/Sustain/Release | advance_to + PhaseDefinitions | pinned phase TSV |
| flows.md orchestration | Flow, FlowStep and bounded cycle detection | flow tests |
| semantic-input.ts/GutTS resolution | InputContract, ModelAdapter, explicit adapter error | input tests |
| React component rendering | external Renderer trait, no React implementation | semantic_consumer example |
| External clock/events/storage/I-O | Clock, EventSource, Storage, ExternalIo | typed host test |

Runtime invariants:
- Every component definition has a nonempty ID, initial state and valid event bindings.
- Scoped addresses are resolved from actual registered parent relationships.
- Event handlers resolve only through the state active when invocation starts.
- State changes/notifications never reborrow a mutable handler. Reentrant actions are
  queued FIFO, and unbounded recursive events are rejected after 1024 executions.
- Observations return deterministic snapshots, sorted by component ID.
- Attack/Release use finite explicit milliseconds; Sustain loops a one-second
  domain. Curves are monotonic, validated integer-valued and deterministic.
- Host adapters receive errors instead of fake native integration.
- Failed handlers do not create successful history entries.
- Theme application clears local visual overrides, matching FrnlTS.

Limitations: no real desktop window, platform accessibility, focus/geometry adapter,
React interoperability or arbitrary Zod validation. Host-provided ModelAdapter is
responsible for candidate inference. The headless engine uses the standard library:
no_std/alloc and compiled GutRS linkage remain future portability work. No real
native OS-level UI integration is claimed by the executable demonstration.
