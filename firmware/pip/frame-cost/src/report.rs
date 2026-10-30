//! Per-scenario, per-row cost report built from the bench's marker events.
use crate::cpu::{mark, Counters, Event, Machine, Stop};
use crate::elf;
use crate::model::{CycleModel, CPU_HZ, MODEL_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Raw counts of one span (a tile row, or a whole frame when the renderer has no per-row call).
#[derive(Clone, Copy, Debug, Default)]
pub struct RowCounts {
    /// Counters accumulated over the span.
    pub counters: Counters,
}

/// One scenario's frames, each a list of row spans.
#[derive(Clone, Debug, Default)]
pub struct ScenarioRun {
    /// Scenario name.
    pub name: String,
    /// `frames[frame][row]`.
    pub frames: Vec<Vec<RowCounts>>,
    /// Frame-buffer checksum the bench reported at the end of each frame.
    pub hashes: Vec<u32>,
}

/// Runs the ELF to completion and groups its marker events.
///
/// # Errors
/// A load failure, a stop other than the program's clean exit, or inconsistent markers.
pub fn run_elf(
    elf_bytes: &[u8],
    model: CycleModel,
    budget: u64,
) -> Result<Vec<ScenarioRun>, String> {
    run_elf_profiled(elf_bytes, model, budget, false).map(|(runs, _)| runs)
}

/// Base cycles per function over the whole run, largest first.
pub type Profile = Vec<(String, u64)>;

/// As [`run_elf`], optionally also attributing base cycles to functions.
///
/// # Errors
/// As [`run_elf`].
pub fn run_elf_profiled(
    elf_bytes: &[u8],
    model: CycleModel,
    budget: u64,
    profile: bool,
) -> Result<(Vec<ScenarioRun>, Profile), String> {
    let mut machine = Machine::new(model);
    elf::load(&mut machine, elf_bytes)?;
    if profile {
        machine.enable_profile();
    }
    match machine.run(budget) {
        Stop::Exit(0) => {}
        other => {
            let trail: Vec<String> = machine
                .recent
                .iter()
                .map(|(pc, w)| format!("{pc:#010x}: {w:#010x}"))
                .collect();
            return Err(format!(
                "bench stopped with {other:?}\nlast instructions:\n{}",
                trail.join("\n")
            ));
        }
    }
    let mut by_function: Profile = Vec::new();
    if let Some(cycles) = &machine.profile {
        for (addr, size, name) in elf::function_symbols(elf_bytes) {
            let first = (addr.wrapping_sub(crate::cpu::IROM_BASE) / 2) as usize;
            let total: u64 = cycles
                .iter()
                .skip(first)
                .take((size as usize).div_ceil(2))
                .sum();
            if total > 0 {
                by_function.push((name, total));
            }
        }
        by_function.sort_by_key(|f| std::cmp::Reverse(f.1));
    }
    Ok((group(&machine.events)?, by_function))
}

fn group(events: &[Event]) -> Result<Vec<ScenarioRun>, String> {
    let mut out: Vec<ScenarioRun> = Vec::new();
    let mut row_start: Option<Counters> = None;
    for e in events {
        match e.kind {
            mark::SCENARIO => out.push(ScenarioRun {
                name: e.name.clone(),
                frames: Vec::new(),
                hashes: Vec::new(),
            }),
            mark::FRAME_BEGIN => out
                .last_mut()
                .ok_or("frame before scenario")?
                .frames
                .push(Vec::new()),
            mark::FRAME_END => out
                .last_mut()
                .ok_or("frame end before scenario")?
                .hashes
                .push(e.value),
            mark::ROW_BEGIN => row_start = Some(e.at),
            mark::ROW_END => {
                let start = row_start.take().ok_or("row end without begin")?;
                out.last_mut()
                    .and_then(|s| s.frames.last_mut())
                    .ok_or("row outside a frame")?
                    .push(RowCounts {
                        counters: e.at.since(&start),
                    });
            }
            _ => {}
        }
    }
    Ok(out)
}

fn ms(cycles: u64) -> f64 {
    cycles as f64 * 1000.0 / CPU_HZ as f64
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Min, mean and max of a series.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stat {
    /// Smallest.
    pub min: f64,
    /// Mean.
    pub mean: f64,
    /// Largest.
    pub max: f64,
}

fn stat(values: &[f64]) -> Stat {
    Stat {
        min: round3(values.iter().copied().fold(f64::INFINITY, f64::min)),
        mean: round3(values.iter().sum::<f64>() / values.len().max(1) as f64),
        max: round3(values.iter().copied().fold(0.0, f64::max)),
    }
}

/// The committed cost of one scenario.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScenarioCost {
    /// Frames measured.
    pub frames: usize,
    /// Compose time of a whole frame, ms.
    pub frame_ms: Stat,
    /// Mean compose time of tile row `r`, ms.
    pub row_ms_mean: Vec<f64>,
    /// Slowest compose time of tile row `r` over the frames, ms.
    pub row_ms_max: Vec<f64>,
    /// Cycles of `[frame][row]`, for a consumer that replays a frame sequence.
    pub row_cycles: Vec<Vec<u64>>,
    /// Mean instructions per frame.
    pub instrs_per_frame: u64,
    /// Mean flash-cache misses per frame (instruction + data).
    pub misses_per_frame: u64,
    /// Mean divide/remainder instructions per frame.
    pub divs_per_frame: u64,
}

/// What the fixture says about how it was produced.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Meta {
    /// Cost-rule version ([`MODEL_VERSION`]).
    pub model_version: u32,
    /// CPU clock the cycles are converted with.
    pub cpu_hz: u64,
    /// The cycle model parameters.
    pub parameters: CycleModel,
    /// Prose: how the miss penalty was fitted.
    pub calibration: String,
    /// Prose: what the numbers are and are not.
    pub scope: String,
}

/// The committed fixture.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Fixture {
    /// How it was produced.
    pub meta: Meta,
    /// Cost per scenario name.
    pub scenarios: BTreeMap<String, ScenarioCost>,
}

/// Costs of every scenario under `model`.
pub fn costs(runs: &[ScenarioRun], model: &CycleModel) -> BTreeMap<String, ScenarioCost> {
    let mut out = BTreeMap::new();
    for run in runs {
        let rows = run.frames.iter().map(Vec::len).max().unwrap_or(0);
        let cycles: Vec<Vec<u64>> = run
            .frames
            .iter()
            .map(|f| f.iter().map(|r| r.counters.cycles(model)).collect())
            .collect();
        let frame_ms: Vec<f64> = cycles.iter().map(|f| ms(f.iter().sum())).collect();
        let row_series = |r: usize| -> Vec<f64> {
            cycles
                .iter()
                .filter_map(|f| f.get(r))
                .map(|&c| ms(c))
                .collect()
        };
        let n = run.frames.len().max(1) as u64;
        let total = |pick: fn(&Counters) -> u64| -> u64 {
            run.frames
                .iter()
                .flatten()
                .map(|r| pick(&r.counters))
                .sum::<u64>()
                / n
        };
        out.insert(
            run.name.clone(),
            ScenarioCost {
                frames: run.frames.len(),
                frame_ms: stat(&frame_ms),
                row_ms_mean: (0..rows).map(|r| stat(&row_series(r)).mean).collect(),
                row_ms_max: (0..rows).map(|r| stat(&row_series(r)).max).collect(),
                row_cycles: cycles,
                instrs_per_frame: total(|c| c.instrs),
                misses_per_frame: total(Counters::misses),
                divs_per_frame: total(|c| c.divs),
            },
        );
    }
    out
}

/// The `scale_milli` that makes the mean frame time of `run` equal `target_ms` under `model`'s miss
/// penalty.
pub fn fit_scale(run: &ScenarioRun, model: &CycleModel, target_ms: f64) -> Option<f64> {
    let frames = run.frames.len() as f64;
    let base: u64 = run
        .frames
        .iter()
        .flatten()
        .map(|r| r.counters.base_cycles)
        .sum();
    let misses: u64 = run
        .frames
        .iter()
        .flatten()
        .map(|r| r.counters.misses())
        .sum();
    if base == 0 || frames == 0.0 {
        return None;
    }
    let target_cycles = target_ms * CPU_HZ as f64 / 1000.0 * frames;
    Some((target_cycles - (misses * u64::from(model.miss_penalty)) as f64) / base as f64 * 1000.0)
}

/// Version for fixtures built now.
pub const fn version() -> u32 {
    MODEL_VERSION
}

/// Outcome of comparing a fresh run with the committed fixture.
#[derive(Debug, Default)]
pub struct Verdict {
    /// Regressions beyond the threshold, or structural mismatches.
    pub failures: Vec<String>,
    /// Improvements and informational differences.
    pub notes: Vec<String>,
}

/// Fails a scenario whose mean or worst frame is more than `threshold_pct` slower than the
/// fixture, or that is missing; notes the ones that got that much faster (the fixture is stale).
pub fn compare(
    fixture: &Fixture,
    now: &BTreeMap<String, ScenarioCost>,
    threshold_pct: f64,
) -> Verdict {
    let mut v = Verdict::default();
    if fixture.meta.model_version != MODEL_VERSION {
        v.failures.push(format!(
            "fixture is model version {}, the simulator is version {}: regenerate it",
            fixture.meta.model_version, MODEL_VERSION
        ));
    }
    for (name, old) in &fixture.scenarios {
        let Some(new) = now.get(name) else {
            v.failures
                .push(format!("scenario {name} is missing from the bench"));
            continue;
        };
        for (what, was, is) in [
            ("mean", old.frame_ms.mean, new.frame_ms.mean),
            ("worst", old.frame_ms.max, new.frame_ms.max),
        ] {
            let change = (is / was - 1.0) * 100.0;
            if change > threshold_pct {
                v.failures.push(format!(
                    "{name}: {what} frame {was:.2} ms -> {is:.2} ms ({change:+.1}%)"
                ));
            } else if change < -threshold_pct {
                v.notes.push(format!(
                    "{name}: {what} frame {was:.2} ms -> {is:.2} ms ({change:+.1}%): faster, refresh the fixture to keep the gate tight"
                ));
            }
        }
    }
    for name in now.keys().filter(|n| !fixture.scenarios.contains_key(*n)) {
        v.failures.push(format!(
            "scenario {name} is not in the fixture: regenerate it"
        ));
    }
    v
}
