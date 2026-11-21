# FrnlTS → FrnlRS parity inventory

Authoritative FrnlTS pinned source:
`dinilmusick/FrnlTS@a9339b9e92f9ef375ac164e274842644dca60d8a`.
Native foundation: `dinilmusick/GutRS@3e4ad1823166b37ddc437aab04277b42c124ce36`.
FrnlRS deliberately ports semantic machinery, **not React**.

| FrnlTS behavior | FrnlRS | Evidence / limits |
| --- | --- | --- |
| Component definitions and state event identifiers | `semantic::ComponentDefinition`, `StateDefinition`, `EventDefinition` | Validated initial state, nonempty names, typed event payload |
| Current-state event resolution | `Runtime::emit`, `register_handler` | State-specific handler test; unknown/mismatched payload error |
| Scope paths and contextual parent inheritance | `mount`, `set_parent`, `path`, `check_path`, `context` | Cycle and scope isolation tests |
| Components, page data and libraries | `set_data`, `set_page_data`, `register_library` | Scoped data and registry tests |
| State temporal phases | `PhaseDefinitions`, `advance_to`, `Visual::Curve` | Finite Attack/Release, looping one-second Sustain, golden phase fixture |
| Themes and visual overrides | `register_theme`, `apply_theme`, `visuals` | State/phase → theme → runtime override, theme clears overrides |
| Event subscription and orchestration | `subscribe`, `unsubscribe`, `bind_flow` | FIFO notifications, reentrant queue and lifetimes |
| Flows and reusable functions | `Flow`, `FlowStep`, `register_function` | Nested flow/cycle checks, call and data branch tests |
| Semantic input resolution | `InputContract`, optional `ModelAdapter` | Deterministic strict input + explicit host fallback/errors |
| Deterministic rendering inputs | `snapshots()`, host `Renderer` trait | Sorted snapshot tests and executable consumer |
| Real UI control availability | Legacy `Surface`, injected callbacks | PR #1 verified baseline; no simulated real UI action |
| Native host clock/input/storage/external I/O | `Clock`, `EventSource`, `Storage`, `ExternalIo` | Executable mock-host integration and explicit failure paths |
| DOM/React hooks/accessibility/focus/geometry | **Not implemented** | Must live in native renderer or future platform adapter |
| Arbitrary JS/Zod validation and browser UI session | **Not implemented** | Domain-specific validators/auth must be host-supplied |
| No-std embedded/bare metal | **Not implemented** | `std`-dependent core; WASI cross-check for portability |
| Full direct GutRS linkage | **Not implemented** | No unverified dependency forced into core |

Tests: `tests/surface_contract.rs`, `tests/semantic_contract.rs`,
`fixtures/frnlts-phases.tsv`, and the `native_consumer` /
`semantic_consumer` runnable examples.

The fixture derives from FrnlTS pinned phase *contracts*. It is not a claim that
TypeScript and Rust were co-executed on automatically generated fixtures.
See [details](SEMANTIC_PARITY.md).

No native window, keyboard/mouse hardware, operating-system interface,
screen-reader, or real deployed app has been integrated or certified.
