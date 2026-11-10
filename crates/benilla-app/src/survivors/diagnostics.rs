//! The FPS counter and the performance diagnostic: a live panel of what costs the most, and a
//! report in `benilla-config/Diagnostics/perf-<time>.txt`, rewritten while it runs.
use super::{
    enemies::{Enemy, Gem},
    hud::HudFont,
    options::Settings,
    Phase,
};
use benilla_world::system_spans::{self as spans, Kind, Sample};
use bevy::{diagnostic::DiagnosticsStore, prelude::*, time::Real};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

const FPS_REFRESH_SECONDS: f32 = 0.25;
const WINDOW_SECONDS: f32 = 1.0;
const SAVE_SECONDS: f32 = 10.0;
const PANEL_SYSTEMS: usize = 12;
const PANEL_STAGES: usize = 6;
const PANEL_NAME_CHARS: usize = 46;
const OVERLAY_Z: i32 = 850;
const HITCH_MS: f32 = 50.0;
const HITCH_SPAN_NANOS: u64 = 20_000_000;
const HITCH_SPANS: usize = 4;
const MAX_HITCHES: usize = 300;
const PROBE_SLEEP_MS: u64 = 5;

/// The longest a plain sleeping thread overslept since last read: a stall it shares with the main
/// thread is process- or system-wide, one it misses is the main thread's own.
static PROBE_GAP_MS: AtomicU32 = AtomicU32::new(0);
static PROBE_RUNNING: AtomicBool = AtomicBool::new(false);

fn start_probe() {
    #[cfg(not(target_arch = "wasm32"))]
    if !PROBE_RUNNING.swap(true, Ordering::Relaxed) {
        let _ = std::thread::Builder::new()
            .name("stall-probe".into())
            .spawn(|| {
                let nap = std::time::Duration::from_millis(PROBE_SLEEP_MS);
                loop {
                    let start = std::time::Instant::now();
                    std::thread::sleep(nap);
                    let gap = start.elapsed().saturating_sub(nap).as_millis() as u32;
                    PROBE_GAP_MS.fetch_max(gap, Ordering::Relaxed);
                }
            });
    }
}

/// One frame over [`HITCH_MS`] and what was happening around it.
struct Hitch {
    at: f32,
    phase: Phase,
    ms: f32,
    probe_ms: u32,
    writing: bool,
    spans: Vec<(String, u64)>,
}

#[derive(Component)]
pub(super) struct FpsText;
#[derive(Component)]
struct PanelRoot;
#[derive(Component)]
struct PanelText;

#[derive(Default)]
struct FpsCounter {
    frames: u32,
    seconds: f32,
    label: String,
}

impl FpsCounter {
    fn frame(&mut self, seconds: f32) {
        self.frames += 1;
        self.seconds += seconds;
        if self.seconds >= FPS_REFRESH_SECONDS {
            let ms = self.seconds * 1000.0 / self.frames as f32;
            self.label = format!("{:.0} FPS · {ms:.1} ms", 1000.0 / ms);
            self.frames = 0;
            self.seconds = 0.0;
        }
    }
}

#[derive(Default)]
struct Total {
    nanos: u64,
    calls: u64,
    /// The longest single run.
    peak_run: u64,
    /// The worst one-second average, in ms per frame.
    peak_window: f64,
}

#[derive(Clone, Copy, Default)]
struct Counts {
    enemies: usize,
    gems: usize,
    entities: usize,
    dot_stacks: usize,
    impact_requests: u64,
    impact_emitted: u64,
}

struct Second {
    at: f32,
    phase: Phase,
    fps: f32,
    mean: f32,
    worst: f32,
    cpu: Option<f64>,
    gpu: Option<f64>,
    counts: Counts,
    /// Keep the hot systems for this second: cumulative tables hide late-run bottlenecks.
    systems: Vec<(String, f64)>,
}

struct Session {
    path: Option<PathBuf>,
    frames: Vec<f32>,
    window: Vec<f32>,
    window_seconds: f32,
    since_save: f32,
    elapsed: f32,
    /// The frames behind `spans`: those of the closed windows.
    measured_frames: usize,
    spans: BTreeMap<(Kind, String), Total>,
    gpu: BTreeMap<String, (f64, u32)>,
    timeline: Vec<Second>,
    hitches: Vec<Hitch>,
    previous_cpu: Option<f64>,
    panel: String,
}

/// The value at `share` of an ascending slice.
fn percentile(sorted: &[f32], share: f32) -> f32 {
    let last = sorted.len().saturating_sub(1);
    sorted
        .get((last as f32 * share).round() as usize)
        .copied()
        .unwrap_or_default()
}

fn short(name: &str) -> String {
    let name = [
        "benilla_app::survivors::",
        "benilla_app::",
        "benilla_world::",
    ]
    .iter()
    .find_map(|prefix| name.strip_prefix(prefix))
    .unwrap_or(name);
    if name.chars().count() <= PANEL_NAME_CHARS {
        return name.to_owned();
    }
    let kept: String = name.chars().take(PANEL_NAME_CHARS - 1).collect();
    format!("{kept}…")
}

impl Session {
    fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            frames: Vec::new(),
            window: Vec::new(),
            window_seconds: 0.0,
            since_save: 0.0,
            elapsed: 0.0,
            measured_frames: 0,
            spans: BTreeMap::new(),
            gpu: BTreeMap::new(),
            timeline: Vec::new(),
            hitches: Vec::new(),
            previous_cpu: None,
            panel: "Collecting diagnostics…".into(),
        }
    }

    fn frame(&mut self, seconds: f32) {
        let ms = seconds * 1000.0;
        self.frames.push(ms);
        self.window.push(ms);
        self.window_seconds += seconds;
        self.since_save += seconds;
        self.elapsed += seconds;
    }

    /// Fold the finished second into the totals and redraw the panel from it.
    fn close_window(
        &mut self,
        samples: Vec<Sample>,
        passes: Vec<(String, f64)>,
        counts: Counts,
        cpu_seconds: Option<f64>,
        phase: Phase,
    ) {
        let frames = self.window.len().max(1) as f64;
        let per_frame = |nanos: u64| nanos as f64 / 1_000_000.0 / frames;
        let mut systems = Vec::new();
        let mut stages = Vec::new();
        for sample in samples {
            let ms = per_frame(sample.nanos);
            match sample.kind {
                Kind::Schedule => stages.push((sample.name.clone(), ms)),
                Kind::Commands => systems.push((format!("{} (commands)", sample.name), ms)),
                Kind::System => systems.push((sample.name.clone(), ms)),
            }
            let total = self.spans.entry((sample.kind, sample.name)).or_default();
            total.nanos += sample.nanos;
            total.calls += sample.calls;
            total.peak_run = total.peak_run.max(sample.peak);
            total.peak_window = total.peak_window.max(ms);
        }
        let gpu = (!passes.is_empty()).then(|| passes.iter().map(|(_, ms)| ms).sum::<f64>());
        for (pass, ms) in passes {
            let total = self.gpu.entry(pass).or_default();
            total.0 += ms;
            total.1 += 1;
        }
        let cpu = self
            .previous_cpu
            .zip(cpu_seconds)
            .map(|(before, now)| (now - before) * 1000.0 / frames);
        self.previous_cpu = cpu_seconds;
        let mean = self.window.iter().sum::<f32>() / frames as f32;
        systems.sort_by(|a, b| b.1.total_cmp(&a.1));
        let second = Second {
            at: self.elapsed,
            phase,
            fps: if mean > 0.0 { 1000.0 / mean } else { 0.0 },
            mean,
            worst: self.window.iter().copied().fold(0.0, f32::max),
            cpu,
            gpu,
            counts,
            systems: systems.iter().take(32).cloned().collect(),
        };
        self.panel = panel(&second, systems, stages);
        self.timeline.push(second);
        self.measured_frames += self.window.len();
        self.window.clear();
        self.window_seconds = 0.0;
    }

    fn report(&self) -> String {
        let mut sorted = self.frames.clone();
        sorted.sort_by(f32::total_cmp);
        let mean = sorted.iter().sum::<f32>() / sorted.len().max(1) as f32;
        let frames = self.measured_frames.max(1) as f64;
        let p99 = percentile(&sorted, 0.99);
        let fps = |ms: f32| if ms > 0.0 { 1000.0 / ms } else { 0.0 };
        let mut out = String::new();
        let _ = writeln!(out, "Azeroth Survivors - Performance report");
        let _ = writeln!(
            out,
            "Duration {:.0} s, {} frames\n\nFRAME TIME\n  average {mean:.2} ms ({:.0} FPS)\n  \
             p50 {:.2} ms, p95 {:.2} ms, p99 {p99:.2} ms (1% low: {:.0} FPS), worst {:.2} ms",
            self.elapsed,
            sorted.len(),
            fps(mean),
            percentile(&sorted, 0.5),
            percentile(&sorted, 0.95),
            fps(p99),
            sorted.last().copied().unwrap_or_default(),
        );
        let table = |out: &mut String, title: &str, kinds: &[Kind]| {
            let _ = writeln!(
                out,
                "\n{title}\n{:>9} {:>9} {:>9} {:>8}  name\n",
                "ms/frame", "1 s peak", "run worst", "calls/f"
            );
            let mut rows: Vec<_> = self
                .spans
                .iter()
                .filter(|((kind, _), _)| kinds.contains(kind))
                .collect();
            rows.sort_by_key(|(_, total)| std::cmp::Reverse(total.nanos));
            for ((kind, name), total) in rows {
                let suffix = if *kind == Kind::Commands {
                    " (commands)"
                } else {
                    ""
                };
                let _ = writeln!(
                    out,
                    "{:>9.3} {:>9.3} {:>9.3} {:>8.1}  {name}{suffix}",
                    total.nanos as f64 / 1_000_000.0 / frames,
                    total.peak_window,
                    total.peak_run as f64 / 1_000_000.0,
                    total.calls as f64 / frames,
                );
            }
        };
        table(
            &mut out,
            "STAGES (each row includes its systems)",
            &[Kind::Schedule],
        );
        if !self.gpu.is_empty() {
            let _ = writeln!(out, "\nGPU PASSES (ms/frame)");
            let mut rows: Vec<_> = self.gpu.iter().collect();
            rows.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0));
            for (pass, (sum, seconds)) in rows {
                let _ = writeln!(out, "{:>9.3}  {pass}", sum / f64::from((*seconds).max(1)));
            }
        }
        table(
            &mut out,
            "SYSTEMS (CPU time summed across all threads)",
            &[Kind::System, Kind::Commands],
        );
        let mut window_table = |title: &str, second: &Second| {
            let _ = writeln!(out, "\n{title}\nt={:.1}s phase={:?} FPS={:.1} enemies={} gems={} entities={}\nms/frame  system",
                second.at, second.phase, second.fps, second.counts.enemies,
                second.counts.gems, second.counts.entities);
            let _ = writeln!(out, "dot_stacks={} impact_requests_total={} impact_emitted_total={}",
                second.counts.dot_stacks, second.counts.impact_requests, second.counts.impact_emitted);
            for (name, ms) in &second.systems {
                let _ = writeln!(out, "{ms:8.3}  {name}");
            }
        };
        if let Some(second) = self.timeline.last() {
            window_table("LATEST SECOND", second);
        }
        if let Some(second) = self.timeline.iter()
            .filter(|s| s.phase == Phase::Playing && s.at >= 10.0 && s.counts.enemies > 0)
            .max_by(|a, b| a.mean.total_cmp(&b.mean)) {
            window_table("SLOWEST COMBAT SECOND (after startup)", second);
        }
        if !self.hitches.is_empty() {
            let _ = writeln!(
                out,
                "\nHITCHES (frames longer than {HITCH_MS:.0} ms)\nt_s,phase,frame_ms,probe_ms,writing,systems"
            );
        }
        for h in &self.hitches {
            let spans: Vec<_> = h
                .spans
                .iter()
                .map(|(name, nanos)| format!("{} {:.0}", short(name), *nanos as f64 / 1_000_000.0))
                .collect();
            let _ = writeln!(
                out,
                "{:.1},{:?},{:.0},{},{},{}",
                h.at,
                h.phase,
                h.ms,
                h.probe_ms,
                if h.writing { "yes" } else { "no" },
                spans.join(" | "),
            );
        }
        let _ = writeln!(
            out,
            "\nTIMELINE\nt_s,phase,fps,mean_ms,worst_ms,cpu_ms,gpu_ms,enemies,gems,entities"
        );
        let cell = |value: Option<f64>| value.map_or(String::new(), |v| format!("{v:.2}"));
        for s in &self.timeline {
            let _ = writeln!(
                out,
                "{:.0},{:?},{:.0},{:.2},{:.2},{},{},{},{},{}",
                s.at,
                s.phase,
                s.fps,
                s.mean,
                s.worst,
                cell(s.cpu),
                cell(s.gpu),
                s.counts.enemies,
                s.counts.gems,
                s.counts.entities,
            );
        }
        out
    }

    fn save(&mut self) {
        self.since_save = 0.0;
        let Some(path) = &self.path else {
            return;
        };
        if let Err(error) = crate::local_state::write_queued(path, &self.report()) {
            warn!("Could not write {}: {error}", path.display());
        }
    }
}

fn panel(
    second: &Second,
    mut systems: Vec<(String, f64)>,
    mut stages: Vec<(String, f64)>,
) -> String {
    let ms = |value: Option<f64>| value.map_or("?".to_owned(), |v| format!("{v:.1} ms"));
    let mut out = format!(
        "PERFORMANCE DIAGNOSTICS · {:.0} s\n{:.0} FPS · average {:.1} ms · worst {:.1} ms\n\
         CPU {} · GPU {}\nEnemies {} · Gems {} · Entities {}\n",
        second.at,
        second.fps,
        second.mean,
        second.worst,
        ms(second.cpu),
        ms(second.gpu),
        second.counts.enemies,
        second.counts.gems,
        second.counts.entities,
    );
    let by_cost = |rows: &mut Vec<(String, f64)>| rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    by_cost(&mut stages);
    by_cost(&mut systems);
    if systems.is_empty() {
        out.push_str(
            "\nPer-system details are unavailable in this build.\nRun warcraft-survivors-perf.exe.",
        );
        return out;
    }
    out.push_str("\nStages (ms/frame)\n");
    for (name, ms) in stages.iter().take(PANEL_STAGES) {
        let _ = writeln!(out, "{ms:6.2}  {}", short(name));
    }
    out.push_str("\nSystems (ms/frame)\n");
    for (name, ms) in systems.iter().take(PANEL_SYSTEMS) {
        let _ = writeln!(out, "{ms:6.2}  {}", short(name));
    }
    out.truncate(out.trim_end().len());
    out
}

#[derive(Resource, Default)]
struct Diagnostics {
    fps: FpsCounter,
    session: Option<Session>,
}

fn report_path() -> Option<PathBuf> {
    let stamp = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |time| time.as_secs());
    crate::local_state::home()
        .map(|home| home.join("Diagnostics").join(format!("perf-{stamp}.txt")))
}

fn gpu_passes(store: Option<&DiagnosticsStore>) -> Vec<(String, f64)> {
    store
        .into_iter()
        .flat_map(|store| store.iter())
        .filter_map(|diagnostic| {
            let path = diagnostic.path().as_str();
            let pass = path.strip_prefix("render/")?.strip_suffix("/elapsed_gpu")?;
            Some((pass.to_owned(), diagnostic.smoothed()?))
        })
        .collect()
}

fn measure(
    time: Res<Time<Real>>,
    settings: Res<Settings>,
    phase: Res<State<Phase>>,
    mut diagnostics: ResMut<Diagnostics>,
    store: Option<Res<DiagnosticsStore>>,
    enemies: Query<&Enemy>,
    gems: Query<(), With<Gem>>,
    entities: Query<Entity>,
    impacts: Option<Res<super::units::VisualImpacts>>,
) {
    let seconds = time.delta_secs();
    diagnostics.fps.frame(seconds);
    if settings.perf_diag && diagnostics.session.is_none() {
        spans::drain();
        spans::set_active(true);
        start_probe();
        diagnostics.session = Some(Session::new(report_path()));
    }
    let stopping = !settings.perf_diag;
    let Some(session) = diagnostics.session.as_mut() else {
        return;
    };
    session.frame(seconds);
    let probe_ms = PROBE_GAP_MS.swap(0, Ordering::Relaxed);
    if seconds * 1000.0 >= HITCH_MS && session.hitches.len() < MAX_HITCHES {
        let mut peaks = spans::peaks(HITCH_SPAN_NANOS);
        peaks.truncate(HITCH_SPANS);
        session.hitches.push(Hitch {
            at: session.elapsed,
            phase: *phase.get(),
            ms: seconds * 1000.0,
            probe_ms,
            writing: crate::local_state::wrote_within(std::time::Duration::from_millis(200)),
            spans: peaks,
        });
    }
    if session.window_seconds >= WINDOW_SECONDS || stopping {
        session.close_window(
            spans::drain(),
            gpu_passes(store.as_deref()),
            Counts {
                enemies: enemies.iter().count(),
                gems: gems.iter().count(),
                entities: entities.iter().count(),
                dot_stacks: enemies.iter().map(|e| e.dots.len()).sum(),
                impact_requests: impacts.as_ref().map_or(0, |v| v.requested),
                impact_emitted: impacts.as_ref().map_or(0, |v| v.emitted),
            },
            crate::perf::process_cpu_secs(),
            *phase.get(),
        );
    }
    if session.since_save >= SAVE_SECONDS || stopping {
        session.save();
    }
    if stopping {
        spans::set_active(false);
        if let Some(path) = diagnostics.session.take().and_then(|s| s.path) {
            info!("Performance report: {}", path.display());
        }
    }
}

fn spawn_overlay(mut commands: Commands, font: Option<Res<HudFont>>, mut done: Local<bool>) {
    let Some(font) = font.filter(|_| !*done) else {
        return;
    };
    *done = true;
    let text = |size| TextFont {
        font: font.0.clone(),
        font_size: size,
        ..default()
    };
    commands
        .spawn((
            PanelRoot,
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(OVERLAY_Z),
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(18.0),
                left: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                PanelText,
                Text::default(),
                text(13.0),
                TextColor(Color::WHITE),
            ));
        });
}

fn show_overlay(
    settings: Res<Settings>,
    diagnostics: Res<Diagnostics>,
    mut fps: Query<(&mut Text, &mut Visibility), (With<FpsText>, Without<PanelText>)>,
    mut root: Query<&mut Visibility, (With<PanelRoot>, Without<FpsText>)>,
    mut panel: Query<&mut Text, (With<PanelText>, Without<FpsText>)>,
) {
    let visible = |on| {
        if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    if let Ok((mut text, mut visibility)) = fps.single_mut() {
        visibility.set_if_neq(visible(settings.show_fps));
        if settings.show_fps && text.0 != diagnostics.fps.label {
            text.0.clone_from(&diagnostics.fps.label);
        }
    }
    if let Ok(mut visibility) = root.single_mut() {
        visibility.set_if_neq(visible(diagnostics.session.is_some()));
    }
    if let (Some(session), Ok(mut text)) = (&diagnostics.session, panel.single_mut()) {
        if text.0 != session.panel {
            text.0.clone_from(&session.panel);
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Diagnostics>()
        .add_systems(Update, (spawn_overlay, measure, show_overlay).chain());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slowest_combat_snapshot_excludes_loading_and_card_pauses() {
        let mut session = Session::new(None);
        for (phase, frames, dt, name) in [
            (Phase::Hub, 100, 0.1, "loading"),
            (Phase::Playing, 40, 0.025, "combat_hotspot"),
            (Phase::LevelUp, 10, 0.1, "card_screen"),
        ] {
            for _ in 0..frames { session.frame(dt); }
            session.close_window(vec![sample(Kind::System, name, 10)], vec![],
                Counts { enemies: 300, ..default() }, None, phase);
        }
        let report = session.report();
        let latest = report.split("LATEST SECOND").nth(1).unwrap()
            .split("SLOWEST COMBAT SECOND").next().unwrap();
        assert!(latest.contains("LevelUp") && latest.contains("card_screen"));
        let combat = report.split("SLOWEST COMBAT SECOND").nth(1).unwrap()
            .split("TIMELINE").next().unwrap();
        assert!(combat.contains("Playing") && combat.contains("combat_hotspot"));
        assert!(!combat.contains("loading") && !combat.contains("card_screen"));
    }

    fn sample(kind: Kind, name: &str, ms: u64) -> Sample {
        Sample {
            kind,
            name: name.into(),
            nanos: ms * 1_000_000,
            calls: 10,
            peak: ms * 200_000,
        }
    }

    #[test]
    fn the_fps_counter_averages_each_refresh() {
        let mut counter = FpsCounter::default();
        for _ in 0..25 {
            counter.frame(0.01);
        }
        assert_eq!(counter.label, "100 FPS · 10.0 ms");
    }

    #[test]
    fn percentiles_read_the_sorted_frames() {
        let sorted: Vec<f32> = (1..=100).map(|n| n as f32).collect();
        assert_eq!(percentile(&sorted, 0.5), 51.0);
        assert_eq!(percentile(&sorted, 0.99), 99.0);
        assert_eq!(percentile(&[], 0.99), 0.0);
    }

    #[test]
    fn names_lose_their_crate_and_stay_short() {
        assert_eq!(
            short("benilla_app::survivors::hud::refresh_hud"),
            "hud::refresh_hud"
        );
        assert_eq!(short(&"x".repeat(80)).chars().count(), PANEL_NAME_CHARS);
    }

    #[test]
    fn a_session_ranks_systems_per_frame_and_keeps_the_worst_second() {
        let mut session = Session::new(None);
        for second in [1, 3] {
            for _ in 0..10 {
                session.frame(0.1);
            }
            session.close_window(
                vec![
                    sample(Kind::System, "benilla_app::survivors::cheap", 10 * second),
                    sample(Kind::System, "benilla_app::survivors::costly", 50 * second),
                    sample(Kind::Commands, "benilla_app::survivors::costly", 20),
                    sample(Kind::Schedule, "Update", 80 * second),
                ],
                vec![("opaque".into(), 2.0), ("ui".into(), 1.0)],
                Counts {
                    enemies: 7,
                    gems: 2,
                    entities: 100,
                    ..default()
                },
                Some(f64::from(second as u32)),
                Phase::Playing,
            );
        }
        let costly = &session.spans[&(Kind::System, "benilla_app::survivors::costly".into())];
        assert_eq!(costly.nanos, 200_000_000);
        assert_eq!(costly.peak_window, 15.0);
        assert!(session.panel.find("costly").unwrap() < session.panel.find("cheap").unwrap());
        assert!(session.panel.contains("GPU 3.0 ms"));
        assert!(session.panel.contains("Enemies 7"));
        // Frames of an open window have no span time yet, so they must not dilute the averages.
        session.frame(0.1);
        let report = session.report();
        assert!(report.contains("   10.000    15.000"));
        let at = |needle: &str| report.find(needle).unwrap();
        assert!(at("FRAME TIME") < at("STAGES") && at("STAGES") < at("GPU PASSES"));
        assert!(at("GPU PASSES") < at("SYSTEMS") && at("SYSTEMS") < at("TIMELINE"));
        assert!(at("survivors::costly\n") < at("survivors::cheap\n"));
        assert!(report.contains("costly (commands)"));
        assert!(report.contains("  2.000  opaque"));
        assert!(report.contains(",Playing,10,100.00,100.00,"));
    }

    #[test]
    fn without_spans_the_panel_says_the_detail_is_missing() {
        let mut session = Session::new(None);
        session.frame(1.0);
        session.close_window(vec![], vec![], Counts::default(), None, Phase::Menu);
        assert!(session.panel.contains("unavailable in this build"));
        assert!(session.panel.contains("CPU ? · GPU ?"));
    }
}
