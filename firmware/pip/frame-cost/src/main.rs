//! `frame-cost`: run the bench ELF in the ISS and report, fit or check ESP32-C3 compose cost.
use kivori_pip_frame_cost::model::{CycleModel, CPU_HZ, MODEL_VERSION};
use kivori_pip_frame_cost::report::{self, Fixture, Meta, ScenarioRun};
use std::process::ExitCode;

const USAGE: &str = "usage:
  frame-cost run --elf ELF [--miss-penalty N] [--scale-milli N] [--scenario NAME] [--out FIXTURE.json]
  frame-cost calibrate --elf ELF --scenario NAME --target-ms MS [--miss-penalty N]
  frame-cost check --elf ELF --fixture FIXTURE.json [--threshold PERCENT]";

const CALIBRATION: &str = "scale_milli fitted so the connected_desk mean is 79 ms (ESP32-C3 rev v0.4 at 160 MHz, firmware after PR #150, `device_probe internals`); miss_penalty assumed (80 MHz quad flash). Validated unfitted on the pre-#150 renderer: predicted 305 ms against 339 ms measured (-10%).";
const SCOPE: &str = "Predicted drawing time of one frame, as Runtime::step measures compose (I/O between rows excluded): the real renderer compiled for riscv32imc (opt-level 3, lto) in an ISS with an assumed ESP32-C3 cycle model. Excludes SPI flush, USB, interrupts and ROM libgcc/libc timing (the bench links compiler-builtins). See docs/virtual-pip.md, Hardware parity.";

/// Instruction budget: far above any bench run, so a runaway program still ends.
const BUDGET: u64 = 20_000_000_000;

struct Args(Vec<String>);

impl Args {
    fn get(&self, flag: &str) -> Option<&str> {
        let i = self.0.iter().position(|a| a == flag)?;
        self.0.get(i + 1).map(String::as_str)
    }
    fn need(&self, flag: &str) -> Result<&str, String> {
        self.get(flag)
            .ok_or_else(|| format!("missing {flag}\n{USAGE}"))
    }
}

fn simulate(args: &Args, model: CycleModel) -> Result<Vec<ScenarioRun>, String> {
    let path = args.need("--elf")?;
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let profile = args.get("--profile").and_then(|n| n.parse::<usize>().ok());
    let (mut runs, by_function) =
        report::run_elf_profiled(&bytes, model, BUDGET, profile.is_some())?;
    if let Some(top) = profile {
        let total: u64 = by_function.iter().map(|f| f.1).sum();
        println!("base cycles by function over the whole run (top {top}):");
        for (name, cycles) in by_function.iter().take(top) {
            let short: String = name.chars().take(90).collect();
            println!("{:>6.2}%  {short}", *cycles as f64 * 100.0 / total as f64);
        }
    }
    if let Some(only) = args.get("--scenario") {
        runs.retain(|r| r.name == only);
        if runs.is_empty() {
            return Err(format!("no scenario {only}"));
        }
    }
    Ok(runs)
}

fn model_from(args: &Args) -> Result<CycleModel, String> {
    let mut model = CycleModel::v1();
    if let Some(p) = args.get("--miss-penalty") {
        model.miss_penalty = p.parse().map_err(|_| "bad --miss-penalty".to_string())?;
    }
    if let Some(p) = args.get("--scale-milli") {
        model.scale_milli = p.parse().map_err(|_| "bad --scale-milli".to_string())?;
    }
    Ok(model)
}

fn print_table(costs: &std::collections::BTreeMap<String, report::ScenarioCost>) {
    println!(
        "{:<16} {:>6} {:>9} {:>9} {:>9} {:>11} {:>9} {:>7}  row ms (mean)",
        "scenario", "frames", "mean ms", "min ms", "max ms", "instr/frame", "miss/frm", "div/frm"
    );
    for (name, c) in costs {
        let rows: Vec<String> = c.row_ms_mean.iter().map(|m| format!("{m:.1}")).collect();
        println!(
            "{:<16} {:>6} {:>9.2} {:>9.2} {:>9.2} {:>11} {:>9} {:>7}  {}",
            name,
            c.frames,
            c.frame_ms.mean,
            c.frame_ms.min,
            c.frame_ms.max,
            c.instrs_per_frame,
            c.misses_per_frame,
            c.divs_per_frame,
            rows.join(" ")
        );
    }
}

fn run(argv: Vec<String>) -> Result<ExitCode, String> {
    let (command, rest) = argv.split_first().ok_or(USAGE)?;
    let args = Args(rest.to_vec());
    match command.as_str() {
        "run" => {
            let model = model_from(&args)?;
            let runs = simulate(&args, model)?;
            let costs = report::costs(&runs, &model);
            print_table(&costs);
            if args.0.iter().any(|a| a == "--hashes") {
                for r in &runs {
                    let list: Vec<String> = r.hashes.iter().map(|h| format!("{h:08x}")).collect();
                    println!("hashes {}: {}", r.name, list.join(" "));
                }
            }
            if args.0.iter().any(|a| a == "--verbose") {
                for r in &runs {
                    let mut t = kivori_pip_frame_cost::cpu::Counters::default();
                    for c in r.frames.iter().flatten() {
                        let c = c.counters;
                        t.instrs += c.instrs;
                        t.base_cycles += c.base_cycles;
                        t.imisses += c.imisses;
                        t.dmisses += c.dmisses;
                        t.flash_loads += c.flash_loads;
                        t.loads += c.loads;
                        t.stores += c.stores;
                        t.muls += c.muls;
                        t.divs += c.divs;
                        t.taken += c.taken;
                    }
                    let n = r.frames.len() as u64;
                    println!("{}: per frame instrs {} base_cycles {} (cpi {:.2}) imiss {} dmiss {} loads {} flash_loads {} stores {} muls {} divs {} taken {}", r.name, t.instrs/n, t.base_cycles/n, t.base_cycles as f64 / t.instrs as f64, t.imisses/n, t.dmisses/n, t.loads/n, t.flash_loads/n, t.stores/n, t.muls/n, t.divs/n, t.taken/n);
                }
            }
            if let Some(out) = args.get("--out") {
                let fixture = Fixture {
                    meta: Meta {
                        model_version: MODEL_VERSION,
                        cpu_hz: CPU_HZ,
                        parameters: model,
                        calibration: CALIBRATION.to_string(),
                        scope: SCOPE.to_string(),
                    },
                    scenarios: costs,
                };
                let text = serde_json::to_string_pretty(&fixture).map_err(|e| e.to_string())?;
                std::fs::write(out, text + "\n").map_err(|e| format!("{out}: {e}"))?;
            }
            Ok(ExitCode::SUCCESS)
        }
        "calibrate" => {
            let model = model_from(&args)?;
            let runs = simulate(&args, model)?;
            let target: f64 = args
                .need("--target-ms")?
                .parse()
                .map_err(|_| "bad --target-ms")?;
            match report::fit_scale(&runs[0], &model, target) {
                Some(s) => println!(
                    "scale_milli to reach {target} ms on {} (miss_penalty {}): {s:.1}",
                    runs[0].name, model.miss_penalty
                ),
                None => println!("nothing to fit in {}", runs[0].name),
            }
            Ok(ExitCode::SUCCESS)
        }
        "check" => {
            let path = args.need("--fixture")?;
            let fixture: Fixture = serde_json::from_str(
                &std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
            )
            .map_err(|e| format!("{path}: {e}"))?;
            let threshold: f64 = args
                .get("--threshold")
                .unwrap_or("10")
                .parse()
                .map_err(|_| "bad --threshold")?;
            let model = fixture.meta.parameters;
            let runs = simulate(&args, model)?;
            let now = report::costs(&runs, &model);
            print_table(&now);
            let verdict = report::compare(&fixture, &now, threshold);
            for line in &verdict.notes {
                println!("{line}");
            }
            if verdict.failures.is_empty() {
                println!("frame-cost: OK (threshold +{threshold}% on mean and worst frame)");
                Ok(ExitCode::SUCCESS)
            } else {
                for line in &verdict.failures {
                    eprintln!("FAIL {line}");
                }
                eprintln!(
                    "frame-cost: predicted compose time regressed. If the cost is intended, run `just fw-frame-cost-update` and commit firmware/pip/frame-cost/fixtures/frame-cost.json with the reason."
                );
                Ok(ExitCode::FAILURE)
            }
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
