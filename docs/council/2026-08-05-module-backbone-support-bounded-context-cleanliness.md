<!--
date: 2026-08-05
repo_type: module
unit: backbone-support
focus: bounded-context-cleanliness (proof obligations: COMPLETENESS + REGENERATION SAFETY)
roster: chair, steelman, skeptic (subagent seats); ddd-bounded-context, yagni-business (in-context)
-->

# Council Report — `backbone-support` bounded-context cleanliness

**Unit:** Metaphor domain module `backbone-support` (type: `module`; bounded context: customer support)
**Focus lens:** bounded-context-cleanliness — proof obligations: (1) COMPLETENESS, (2) REGENERATION SAFETY
**Date:** 2026-08-05
**Chair's call:** Do **not** sign off on regen-safety yet. Run a sentinel round-trip probe as the gating move; ship thereafter.

---

## BEST CALL (single move)

**Run a sentinel round-trip probe across all four preservation-marker dialects before trusting regen with any hand-written custom logic.** Drop a unique sentinel comment + a sentinel fn/statement inside one representative block of EACH of the four dialects plus a canonical `// <<< CUSTOM` / `// END CUSTOM` control, run `metaphor schema generate --force`, and `git diff` for the sentinels.

- **Reversibility:** Full. ~5 min on a scratch branch; `git checkout .` restores state.
- **Residual negative value if probe is run:** ~5 min engineer time + one throwaway branch. No production surface touched.
- **Residual negative value if probe is SKIPPED (i.e. ship as-is):** Unbounded and silent. The first developer who fills a `// <<< CUSTOM METHODS START >>>` block (the *documented* entity-extension pattern — `issue.rs:286`, `sla.rs:154`, `slp.rs:161`, `wc.rs:236`) loses the contents on the next regen with **no warning, green build, no test signal** — exactly the failure mode already empirically observed at `src/application/service/issue_domain_policy.rs:11-18` for non-marker content. The four dialects (`CUSTOM METHODS`, `CUSTOM COMPAT`, `CUSTOM DTOs`, `CUSTOM RULES`) are all currently **empty**, and an empty block round-trips identically whether the generator's preservation regex recognizes it or not — so the green diff observed is *non-evidence* for preservation correctness.
- **Evidence that would flip this call to "ship now, no probe":** A citation in the `metaphor-codegen` plugin source (or its preservation-contract spec) that enumerates all four marker dialects as recognized boundaries — not just the canonical `// <<< CUSTOM` / `// END CUSTOM`. If the generator only keys on the canonical form, the other three dialects are decorative comments and any code placed inside them is dead on arrival. A doc check (~2 min) is cheaper than the empirical probe but proves only what the docs claim, not what the binary does; the sentinel probe is definitive.

**Rationale for picking the probe over "ship now":** The completeness obligation is met (see below); the regen-safety obligation is **not** met by the evidence on hand. The user's stated second obligation is *future regens won't destroy hand-edits* — empty-block round-trip cannot discharge that obligation, and the cost of being wrong is silent data loss with a green build. Five minutes of probe is the cheapest insurance that exists.

---

## Completeness verdict (obligation 1) — DISCHARGED

No probe needed; this is direct from the tree:

- 4 real entities (`Issue`, `ServiceLevelAgreement`, `ServiceLevelPriority`, `WarrantyClaim`) with full 4-layer fanout.
- All 4 services registered in `SupportModuleBuilder::build()` (`src/lib.rs:148-179`), Arc-held on `SupportModule` (`src/lib.rs:56-63`).
- Three deliberate, documented route shapes (`all_crud_routes`, `readonly_routes`, `#[deprecated] routes`) — the deprecation note at `src/lib.rs:96` is safety-positive, not tech debt.
- Multi-model YAML (`service_level_agreement.model.yaml` holds SLA + SLP) produced all artifacts for both models.
- No dangling `SupportQueryService`/`QueryServiceImpl` references.

---

## MUST-FIX before ship (independent of the probe)

**Delete the 8 orphan `example_*` files.** Confirmed via glob: `schema/models/example.model.yaml` **does not exist** — the source of truth was already cleansed. The generated orphans are dead code:
- `src/domain/entity/example.rs`, `src/domain/repositories/example_repository.rs`, `src/infrastructure/persistence/example_repository_impl.rs`, `src/application/service/example_service.rs`, `src/application/dto/example_dto.rs`, `src/presentation/http/example_handler.rs`, `src/routes/example_routes.rs`, `src/seeders/example_seeder.rs`.

Two reasons this is non-negotiable even though the build is green today:

1. **Build is green only by accident.** These files are not declared in `entity/mod.rs`, `application/service/mod.rs`, `presentation/http/mod.rs`, or `routes/mod.rs` (verified). `example_handler.rs` and `routes/example_routes.rs` reference `module.example_service`, which does **not** exist on `SupportModule`. The instant any developer (or a future regen that re-introduces `example.model.yaml`) re-declares any of these modules, the build breaks with a confusing "no field `example_service` on `SupportModule`" error pointing at files they didn't touch.
2. **Bounded-context leak.** A shipped module exporting an `Example` aggregate pollutes the support context's surface. The ddd-bounded-context seat's defect call is correct; this is not a style nit.

Cost: ~2 min, zero risk, no probe required. Add `example*` to nothing — just delete. Re-verify `cargo build` after.

---

## Disagreement map

| Question | Steelman | Skeptic | Chair adjudication |
|---|---|---|---|
| Is the just-run regen diff evidence of preservation working? | "Yes — diff is entirely generator-owned; nothing hand-written was touched." | "No — empty blocks round-trip identically whether the regex matched or not; the diff is non-evidence for preservation correctness." | **Skeptic.** The steelman's claim is factually true but answers a different question ("was anything hand-edited clobbered *this* run?") than the obligation asks ("will future regens preserve hand-edits?"). |
| Is the multi-model YAML safe across regen? | "Medium confidence — generator honored it once." | "Same non-evidence problem: one successful regen of an unchanged file proves nothing about determinism under edit." | **Skeptic, partially.** The probe covers this for free if the sentinel run also re-runs SLA/SLP generation and the diff is inspected for unintended changes. |
| Are the example orphans a real defect? | (Not contested.) | (Not contested.) | **Both seats agree; ddd-bounded-context flagged it.** Not a disagreement — just land it. |
| Is the module over-built? | yagni-business: no over-build; regen's value this month is real (QueryServiceImpl fix, readonly_routes, invariant-bypass doc warnings). | (Concurs.) | **Consensus: ship the shape.** No work to do here. |

---

## Ranked alternatives to the Best call

1. **(Best) Sentinel round-trip probe across all 4 dialects + canonical control.** ~5 min, reversible, definitive. Becomes the ship gate.
2. **Doc/source probe only.** Grep `metaphor-codegen` plugin for the four marker literals; if all four are enumerated as preservation boundaries, ship without the empirical run. ~2 min, weaker evidence (docs vs. binary).
3. **Standardize on `*_custom.rs` sibling files only; abandon inline markers entirely.** Move every future hand-edit into `*_custom.rs` files (already supported, already in `user_owned`). Zero probe cost; narrows the developer's extension vocabulary to one safe path. **Cost:** loses the inline-extension ergonomics the entity `// <<< CUSTOM METHODS START >>>` block was designed for; re-educate developers; refactor any future inline use out. Viable long-term hedge if the probe fails.
4. **Ship as-is, no probe, no fix.** **Rejected.** Carries the silent-loss risk above plus ships a bounded-context leak. Save zero minutes worth saving.

---

## Parking lot (out of this focus lens)

- **Generator-side fix:** the four dialects should be either (a) canonicalized to one form by the codegen plugin, or (b) explicitly enumerated in the preservation contract. File against `metaphor-codegen`, not this module.
- **Deprecation timeline for `SupportModule::routes()`:** currently just `#[deprecated]`. Decide a removal version so the unguarded funnel doesn't live forever.
- **Multi-model YAML determinism:** add a codegen test that regenerates a known multi-model fixture and asserts byte-stable output. Belongs in the codegen plugin's test suite.
- **`#[allow(unused_imports)]` at `src/lib.rs:18`:** broad suppression; audit at next lint pass.

---

## Probe results — CORRECTED (re-runs on 2026-08-05)

> **Heads-up: the original probe below was confounded and its conclusion retracted.** The
> "per-generator, overwrite wholesale" finding that drove ADR-0012 was an artefact of using
> **comment-only** sentinels. Read this corrected section; ADR-0013 supersedes ADR-0012.

### Original probe (confounded)

A `// SENTINEL-…` *comment* was placed in one representative block per dialect, then
`metaphor schema generate --force` was run. Result observed at the time: **5 of 5 comment sentinels
wiped, including the canonical control.** A follow-up comment-vs-code probe then placed a real
`pub mod …` / `fn …` sentinel in `service/mod.rs` and `handlers/custom.rs`, which **survived**.

That comment-vs-code split was the clue that the variable was not the generator but the *content
type*.

### Why comment-only blocks are dropped — by design

Reading the merge source (`metaphor-plugin-schema/src/commands/schema/merge/custom_blocks/`):

- **One central merger** — every generated `.rs` file flows through `merge_rust_mod_custom`
  (`mod.rs:43`) via `route_merge` in `generate/write.rs`. There is no per-generator write path.
- **Comment-only filtering is intentional:**
  - `single_marker.rs:87-94` — a block is collected only if it has a line that is not a marker, not
    empty, and not a `//` comment.
  - `paired_methods.rs:34-42` — a paired block is preserved only if it has non-comment code
    (a `/// ` doc comment or `TODO` also counts).
  - Pinned by test `mod.rs:196` (`test_comment_only_custom_block_not_duplicated`): placeholder
    comments must not be duplicated on every regen.
- All marker dialects are recognized (`markers.rs:37-71`): canonical `// <<< CUSTOM`/`// END CUSTOM`,
  named `… START >>>`/`… END >>>`, and any line containing `END CUSTOM`.

### Re-probe with REAL code (2026-08-05, controlled)

A real, non-comment sentinel — `fn _probe_<dialect>() {}` — was placed inside one representative
block per generator/dialect, then `metaphor schema generate --force` was run.
**All five survived:**

| Generator (target) | File | Marker dialect | Real-code sentinel survived? |
|---|---|---|---|
| `module` | `lib.rs` | canonical `// <<< CUSTOM` / `// END CUSTOM` | ✅ Yes |
| `entity` | `domain/entity/issue.rs` | `CUSTOM METHODS START >>>` / `END >>>` | ✅ Yes |
| `validator` | `application/validator/issue_validator.rs` | `CUSTOM RULES` / `END CUSTOM RULES` | ✅ Yes |
| `dto` | `presentation/dto/issue_dto.rs` | `CUSTOM DTOs` / `>>> END CUSTOM DTOs` | ✅ Yes |
| `versioning` | `presentation/versioning/version_compat.rs` | `CUSTOM COMPAT` / `>>> END CUSTOM COMPAT` | ✅ Yes |

(Earlier confirmed: `service` canonical and `handlers` `CUSTOM HANDLERS` blocks preserve real code.)

### Corrected conclusion

1. **Real code inside any recognized `CUSTOM` block IS preserved on regen, in every generator.**
   There is no codegen parity bug. The "non-canonical dialects are decorative" claim is **retracted**.
2. **Comment-only blocks are dropped by design** — the one genuine footgun. Do not rely on a
   `CUSTOM` block whose only content is `//` placeholder comments.
3. **`*_custom.rs` siblings and `user_owned` files remain the recommended home for substantial
   custom logic** — as style, not correctness (they are never touched by the generator).

### Follow-on

- **ADR-0012** (which encoded the confounded finding) is now `Superseded by ADR-0013`.
- **ADR-0013** records the live contract: preservation is centralized and works for real code.
- No codegen change is required.

### Action taken this session (still valid)

8 orphan `example_*` files deleted (+ the stray `pub mod example_dto;` declaration in
`application/dto/mod.rs`); regen confirmed it does NOT resurrect them (no `example` in schema).
`cargo build` green. That cleanup stands regardless of the probe correction.

## Relevant files

- `src/lib.rs` — module wiring, route surfaces (lines 56-63, 76-119, 148-179).
- `src/domain/entity/mod.rs` — confirms `example` is NOT declared (clean), 4 real entities declared.
- `src/presentation/http/mod.rs` — confirms `example_handler` not declared.
- `src/application/service/mod.rs` — canonical `// <<< CUSTOM` block with real hand-written modules (`support_events`, `support_ports`, `support_write_service`) — canonical-dialect control site for the probe.
- Marker dialect sites (probe targets):
  - `src/domain/entity/issue.rs:286-287` (`CUSTOM METHODS START/END`)
  - `src/presentation/versioning/version_compat.rs:86,89` (`CUSTOM COMPAT` / `>>> END CUSTOM COMPAT`)
  - `src/presentation/dto/issue_dto.rs:421,424` (`CUSTOM DTOs` / `>>> END CUSTOM DTOs`)
  - `src/application/validator/issue_validator.rs:21-22` (`CUSTOM RULES` / `END CUSTOM RULES`)
- Orphan files to delete (8): `src/domain/entity/example.rs`, `src/domain/repositories/example_repository.rs`, `src/infrastructure/persistence/example_repository_impl.rs`, `src/application/service/example_service.rs`, `src/application/dto/example_dto.rs` (also remove its `pub mod example_dto;` declaration in `src/application/dto/mod.rs`), `src/presentation/http/example_handler.rs`, `src/routes/example_routes.rs`, `src/seeders/example_seeder.rs`.
- Schema source of truth (confirms `example` already removed): `schema/models/` — 4 entity YAMLs + `index.model.yaml`, no `example.model.yaml`.
