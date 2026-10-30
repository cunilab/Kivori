//! Fixed-seed replay through the same raw-input/runtime path as the interactive runner.
use super::*;
use std::collections::BTreeMap;

const GOLDENS: &str = include_str!("../../fixtures/replay-goldens.json");
const IDS: [&str; 4] = [
    "mixed-rotary",
    "invalid-bounce-then-detent",
    "buffered-switches",
    "recovery-cancel",
];

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Goldens {
    schema_version: u8,
    seeds: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    control: String,
    kind: String,
    device_ms: u32,
}

fn events(seen: &[Seen]) -> Vec<Event> {
    seen.iter()
        .map(|Seen(control, kind, device_ms)| Event {
            control: format!("{control:?}"),
            kind: format!("{kind:?}"),
            device_ms: *device_ms,
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawEdge {
    at_ms: u32,
    levels: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TickInput {
    now_ms: u32,
    edges: Vec<RawEdge>,
}

fn tick(now_ms: u32, edges: &[(u32, u8)]) -> TickInput {
    TickInput {
        now_ms,
        edges: edges
            .iter()
            .map(|&(at_ms, levels)| RawEdge { at_ms, levels })
            .collect(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Handshake {
    nonce: u32,
    double_press: bool,
    hello_at_ms: u32,
    ready_at_ms: u32,
}

#[derive(Serialize)]
struct Inputs {
    handshake: Handshake,
    ticks: Vec<TickInput>,
}

#[derive(PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    actions: Vec<Event>,
    frame_hash: String,
    reboot: bool,
}

#[derive(Serialize)]
struct Case {
    id: &'static str,
    status: &'static str,
    inputs: Inputs,
    expected: Observation,
    actual: Observation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u8,
    product: &'static str,
    firmware_version: String,
    seed: u32,
    clock_mode: &'static str,
    frame_hash_algorithm: &'static str,
    frame_width: u16,
    frame_height: u16,
    simulated_layers: [&'static str; 4],
    exercised_layers: [&'static str; 3],
    not_exercised: [&'static str; 3],
    golden_fixture: &'static str,
    status: &'static str,
    cases: Vec<Case>,
}

fn execute(
    id: &'static str,
    ticks: Vec<TickInput>,
    expected: Vec<Seen>,
    frame_hash: &str,
) -> Result<Case, String> {
    let mut rig = Rig::new(false)?;
    for input in &ticks {
        let edges: Vec<_> = input
            .edges
            .iter()
            .map(|edge| (edge.at_ms, edge.levels))
            .collect();
        rig.step(input.now_ms, &edges)
            .map_err(|error| format!("replay {id}: {error}"))?;
    }
    let expected = Observation {
        actions: events(&expected),
        frame_hash: frame_hash.into(),
        reboot: false,
    };
    let actual = Observation {
        actions: events(&rig.events),
        frame_hash: format!("{:016x}", rig.frame_hash),
        reboot: rig.sim.rebooted,
    };
    Ok(Case {
        id,
        status: if expected == actual {
            "passed"
        } else {
            "failed"
        },
        inputs: Inputs {
            handshake: Handshake {
                nonce: 42,
                double_press: false,
                hello_at_ms: 0,
                ready_at_ms: 1,
            },
            ticks,
        },
        expected,
        actual,
    })
}

fn parse_seed(seed: &str) -> Result<u32, String> {
    match seed.parse() {
        Ok(seed @ (17 | 29)) => Ok(seed),
        _ => Err("replay supports only fixed seeds 17 and 29".into()),
    }
}

fn report(seed: u32, goldens: Goldens) -> Result<Report, String> {
    std::thread::Builder::new()
        .name("firmware-replay".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(move || scenarios(seed, &goldens))
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "firmware replay thread panicked".to_string())?
}

fn scenarios(seed: u32, goldens: &Goldens) -> Result<Report, String> {
    require(
        goldens.schema_version == 1,
        "unsupported replay golden schema",
    )?;
    require(
        goldens.seeds.len() == 2
            && goldens.seeds.contains_key("17")
            && goldens.seeds.contains_key("29"),
        "golden seed inventory does not match fixed replay seeds",
    )?;
    let hashes = goldens
        .seeds
        .get(&seed.to_string())
        .ok_or("missing replay seed goldens")?;
    require(
        hashes.len() == IDS.len() && IDS.iter().all(|id| hashes.contains_key(*id)),
        "golden scenario inventory does not match replay recipes",
    )?;
    require(
        hashes.values().all(|hash| {
            hash.len() == 16
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }),
        "invalid replay frame hash",
    )?;
    let base = 100 + seed % 31;
    let mut cases = Vec::new();
    let mut raw = Vec::new();
    let mut expected = vec![rotary(InputKind::GestureStarted, base + 3)];
    let mut state = seed;
    for index in 0..4 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let (phases, direction) = if state >> 16 & 1 == 0 {
            ([2, 3, 1, 0], Direction::Cw)
        } else {
            ([1, 3, 2, 0], Direction::Ccw)
        };
        let at = base + index * 12;
        raw.extend(
            phases
                .into_iter()
                .enumerate()
                .map(|(i, phase)| (at + i as u32, phase)),
        );
        expected.push(rotary(InputKind::Detent(direction), at + 3));
    }
    let end = base + 39;
    expected.push(rotary(InputKind::GestureEnded, end + 251));
    cases.push(execute(
        IDS[0],
        vec![tick(end + 1, &raw), tick(end + 251, &[])],
        expected,
        &hashes[IDS[0]],
    )?);

    let mut raw: Vec<_> = [3, 1, 0, 2, 0, 2, 0]
        .into_iter()
        .enumerate()
        .map(|(i, phase)| (base + i as u32, phase))
        .collect();
    raw.extend([
        (base + 50, 2),
        (base + 51, 3),
        (base + 52, 1),
        (base + 53, 0),
    ]);
    cases.push(execute(
        IDS[1],
        vec![tick(base + 54, &raw), tick(base + 304, &[])],
        vec![
            rotary(InputKind::GestureStarted, base + 53),
            rotary(InputKind::Detent(Direction::Cw), base + 53),
            rotary(InputKind::GestureEnded, base + 304),
        ],
        &hashes[IDS[1]],
    )?);

    cases.push(execute(
        IDS[2],
        vec![
            tick(
                base + 720,
                &[(base, 4), (base + 120, 0), (base + 170, 8), (base + 670, 0)],
            ),
            tick(base + 820, &[]),
        ],
        vec![
            button(InputKind::Press, base + 170),
            seen(ControlId::ContextButton(0), InputKind::Hold, base + 720),
        ],
        &hashes[IDS[2]],
    )?);

    cases.push(execute(
        IDS[3],
        vec![
            tick(base + 2_000, &[(base, 4)]),
            tick(base + 10_050, &[(base + 2_100, 0)]),
            tick(base + 10_200, &[]),
        ],
        vec![],
        &hashes[IDS[3]],
    )?);
    let passed = cases.iter().all(|case| case.status == "passed");
    Ok(Report {
        schema_version: 1,
        product: "pip",
        firmware_version: format!("{}.{}.{}", VERSION.major, VERSION.minor, VERSION.patch),
        seed,
        clock_mode: "injected-firmware",
        frame_hash_algorithm: "fnv1a64-rgb565-le",
        frame_width: 240,
        frame_height: 240,
        simulated_layers: [
            "normalized-input-levels",
            "clock",
            "transport",
            "display-capture",
        ],
        exercised_layers: ["firmware-runtime", "protocol", "firmware-renderer"],
        not_exercised: ["desktop-runtime", "native-os", "electrical-timing"],
        golden_fixture: "firmware/pip/simulator/fixtures/replay-goldens.json",
        status: if passed { "passed" } else { "failed" },
        cases,
    })
}

pub fn run(seed: &str) -> Result<(), String> {
    let seed = parse_seed(seed)?;
    let goldens: Goldens = serde_json::from_str(GOLDENS).map_err(|error| error.to_string())?;
    let report = report(seed, goldens)?;
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|error| error.to_string())?
    );
    require(
        report.status == "passed",
        &format!("replay seed {seed} failed; inspect expected/actual JSON"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_seed_replays_match_checked_in_actions_and_frames() {
        for seed in [17, 29] {
            let report = report(seed, serde_json::from_str(GOLDENS).unwrap()).unwrap();
            assert_eq!(
                report.status,
                "passed",
                "{}",
                serde_json::to_string(&report).unwrap()
            );
        }
    }

    #[test]
    fn changed_frame_golden_is_rejected_by_the_real_replay() {
        let mut goldens: Goldens = serde_json::from_str(GOLDENS).unwrap();
        goldens
            .seeds
            .get_mut("17")
            .unwrap()
            .insert(IDS[0].into(), "0000000000000000".into());
        let report = report(17, goldens).unwrap();
        assert_eq!(report.status, "failed");
        assert_eq!(report.cases[0].status, "failed");
        assert_ne!(
            report.cases[0].expected.frame_hash,
            report.cases[0].actual.frame_hash
        );
        assert_eq!(
            report.cases[0].expected.actions,
            report.cases[0].actual.actions
        );
        assert!(report.cases[1..].iter().all(|case| case.status == "passed"));
    }

    #[test]
    fn changed_expected_action_is_rejected_after_actual_raw_input() {
        let mut baseline = report(17, serde_json::from_str(GOLDENS).unwrap()).unwrap();
        assert_eq!(baseline.status, "passed");
        let valid = baseline.cases.remove(0);
        let case = execute(
            valid.id,
            valid.inputs.ticks,
            vec![],
            &valid.actual.frame_hash,
        )
        .unwrap();
        assert_eq!(case.status, "failed");
        assert!(!case.actual.actions.is_empty());
        assert_ne!(case.expected.actions, case.actual.actions);
        assert_eq!(case.expected.frame_hash, case.actual.frame_hash);
    }

    #[test]
    fn unsupported_seed_and_malformed_goldens_fail_closed() {
        for seed in ["0", "18", "4294967296", "abc"] {
            assert!(parse_seed(seed).is_err());
        }
        let mut goldens: Goldens = serde_json::from_str(GOLDENS).unwrap();
        goldens.seeds.get_mut("17").unwrap().remove(IDS[0]);
        assert!(report(17, goldens).is_err());
    }
}
