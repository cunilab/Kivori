# Personality-layer review against PRD §9.5

**Date:** 2026-09-25 · **Method:** code review of `crates/kivori-model/src/mascot.rs` (sprite mapping,
animator) and `apps/desktop/src-tauri/src/companion.rs` (self-play). No panel was needed.
**Rule under test:** personality may animate freely, but must never look like a desktop state, and it
always gives way to the higher layers.

## Result: violations found and fixed on 2026-09-25

| Check | Result | Evidence |
|---|---|---|
| Self-play only runs in calm states | ✅ | `CompanionDirector::poll` skips Booting, Busy, Sleeping and Offline; self-play fires only in Idle or Happy |
| A state change interrupts a running reaction | ✅ | `MascotAnimator::set_state` clears `action` and starts the normal eased transition |
| Personality changes motion strength, never the reported state | ✅ | `MascotPersonality` scales amplitude and the self-play interval (Calm 18–30 s, Cozy 8–16 s, Playful 4–8 s) and nothing else |
| **Greet** reused the Happy face | ❌ → ✅ fixed | `Delighted` had Happy's eye and mouth frames `(2,2)`. It is also used by the **idle animation**, so an Idle buddy showed the exact Happy face for 900 ms at random, even with self-play off. It is now `(1,2)`: Idle's open eyes with the open smile |
| **Surprise** reused the Booting face | ❌ → ✅ fixed | `Surprised` had Booting's frames `(0,0)`. It is now `(1,0)`: sparkly eyes with the small "o" mouth |
| Partial overlaps | ⚠️ acceptable | `Reassuring` shares Busy's mouth; `Affectionate` and `Laughing` share Happy's mouth or eyes. Each has at least one unique part |
| Manual reactions over Busy | ❌ → ✅ fixed | `MascotAnimator::trigger_action` now refuses a reaction over Busy, Booting or Offline (`CompanionState::allows_reaction`). The firmware sends no `MascotActionApplied` for a refused reaction, so Desktop never believes it played. The Device Studio preview uses the same animator, so preview and panel agree |
| Manual reactions over Sleeping | ✅ kept (PR #3 design) | A deliberate poke gives a gentle, sleepy `Affectionate` response with its own unique face, then returns to Sleeping. It does not claim a different state |

## Guards added

- `crates/kivori-model/tests/mascot.rs::no_reaction_face_matches_a_state_face`: no reaction's
  `(eye_frame, mouth_frame)` may equal any state's.
- `…::reactions_are_ignored_over_meaningful_states`
- `firmware/esp32-c3/tests/production_runtime.rs::social_action_over_busy_is_refused_unacknowledged_and_draws_nothing`.
  Mutation-checked: removing the firmware gate makes it fail.
- Golden frames: only the two Idle frames inside the idle expression window changed
  (`Idle@21377`, `Idle@21457`); every other reviewed frame is identical.

## Still to do

- Look at the recombined Greet and Surprise faces on the physical panel (Phase 2 physical row).
- When v1 adds Success and Error primary states (PRD §9.3), rerun this review; the face-uniqueness
  test then covers them automatically.
