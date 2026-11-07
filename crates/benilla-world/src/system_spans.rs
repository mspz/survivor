//! Time spent in every ECS system, read from bevy's `trace` spans while a measurement is active.
//!
//! The layer costs one atomic load per span when idle. Without bevy's `trace` feature no span
//! exists and [`drain`] stays empty.
use bevy::log::tracing_subscriber::{field::Visit, layer::Context, registry::LookupSpan, Layer};
use bevy::platform::time::Instant;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        LazyLock, Mutex,
    },
};
use tracing_core::{
    field::Field,
    span::{Attributes, Id},
    Subscriber,
};

/// Far above the few thousand systems of the client; a span past it goes unmeasured.
const CAPACITY: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    System,
    /// A system's deferred commands, applied after it ran.
    Commands,
    /// A whole schedule, which includes the systems inside it.
    Schedule,
}

/// What one span cost since the last [`drain`].
#[derive(Debug)]
pub struct Sample {
    pub kind: Kind,
    pub name: String,
    pub nanos: u64,
    pub calls: u64,
    /// The longest single run.
    pub peak: u64,
}

#[derive(Default)]
struct Slot {
    nanos: AtomicU64,
    calls: AtomicU64,
    peak: AtomicU64,
}

#[derive(Default)]
struct Names {
    index: HashMap<(Kind, String), u32>,
    all: Vec<(Kind, String)>,
}

struct Table {
    slots: Box<[Slot]>,
    names: Mutex<Names>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| Table {
    slots: (0..CAPACITY).map(|_| Slot::default()).collect(),
    names: Mutex::default(),
});
static ACTIVE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static OPEN: RefCell<Vec<(u32, Instant)>> = const { RefCell::new(Vec::new()) };
}

pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

/// Everything measured since the last call, resetting the counters.
pub fn drain() -> Vec<Sample> {
    let names = TABLE.names.lock().unwrap_or_else(|e| e.into_inner());
    names
        .all
        .iter()
        .zip(&TABLE.slots)
        .filter_map(|((kind, name), slot)| {
            let calls = slot.calls.swap(0, Ordering::Relaxed);
            let nanos = slot.nanos.swap(0, Ordering::Relaxed);
            let peak = slot.peak.swap(0, Ordering::Relaxed);
            (calls > 0).then(|| Sample {
                kind: *kind,
                name: name.clone(),
                nanos,
                calls,
                peak,
            })
        })
        .collect()
}

/// The spans whose longest run since the last [`drain`] reached `min_nanos`, longest first.
pub fn peaks(min_nanos: u64) -> Vec<(String, u64)> {
    let names = TABLE.names.lock().unwrap_or_else(|e| e.into_inner());
    let mut peaks: Vec<_> = names
        .all
        .iter()
        .zip(&TABLE.slots)
        .filter(|((kind, _), _)| *kind != Kind::Schedule)
        .filter_map(|((_, name), slot)| {
            let peak = slot.peak.load(Ordering::Relaxed);
            (peak >= min_nanos).then(|| (name.clone(), peak))
        })
        .collect();
    peaks.sort_by_key(|(_, peak)| std::cmp::Reverse(*peak));
    peaks
}

fn intern(kind: Kind, name: String) -> Option<u32> {
    let mut names = TABLE.names.lock().unwrap_or_else(|e| e.into_inner());
    let key = (kind, name);
    if let Some(&slot) = names.index.get(&key) {
        return Some(slot);
    }
    let slot = names.all.len();
    if slot >= CAPACITY {
        return None;
    }
    names.all.push(key.clone());
    names.index.insert(key, slot as u32);
    Some(slot as u32)
}

struct Tag(u32);

#[derive(Default)]
struct NameField(Option<String>);

impl Visit for NameField {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_owned());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "name" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

pub struct SpanTimer;

impl<S> Layer<S> for SpanTimer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let kind = match attrs.metadata().name() {
            "system" => Kind::System,
            "system_commands" => Kind::Commands,
            "schedule" => Kind::Schedule,
            _ => return,
        };
        let mut name = NameField::default();
        attrs.record(&mut name);
        let slot = name.0.and_then(|name| intern(kind, name));
        if let (Some(slot), Some(span)) = (slot, ctx.span(id)) {
            span.extensions_mut().insert(Tag(slot));
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if !ACTIVE.load(Ordering::Relaxed) {
            return;
        }
        let Some(span) = ctx.span(id) else {
            return;
        };
        let Some(slot) = span.extensions().get::<Tag>().map(|tag| tag.0) else {
            return;
        };
        let _ = OPEN.try_with(|open| open.borrow_mut().push((slot, Instant::now())));
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let now = Instant::now();
        let closed = OPEN.try_with(|open| {
            let mut open = open.borrow_mut();
            let (slot, _) = *open.last()?;
            let span = ctx.span(id)?;
            let tag = span.extensions().get::<Tag>()?.0;
            (tag == slot).then(|| open.pop()).flatten()
        });
        if let Ok(Some((slot, start))) = closed {
            let nanos = now.duration_since(start).as_nanos() as u64;
            let slot = &TABLE.slots[slot as usize];
            slot.nanos.fetch_add(nanos, Ordering::Relaxed);
            slot.calls.fetch_add(1, Ordering::Relaxed);
            slot.peak.fetch_max(nanos, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::log::tracing_subscriber::{layer::SubscriberExt, Registry};
    use std::time::Duration;

    #[test]
    fn spans_are_timed_only_while_active_and_a_drain_resets_them() {
        let subscriber = Registry::default().with(SpanTimer);
        tracing::subscriber::with_default(subscriber, || {
            let system = tracing::info_span!("system", name = "test::timed".to_string());
            let schedule = tracing::info_span!("schedule", name = ?"Update");
            let other = tracing::info_span!("other", name = "test::ignored");
            let run = || {
                let _schedule = schedule.enter();
                let _other = other.enter();
                let _system = system.enter();
                std::thread::sleep(Duration::from_millis(2));
            };
            run();
            assert!(!drain().iter().any(|s| s.name == "test::timed"));
            set_active(true);
            run();
            run();
            set_active(false);
            let samples = drain();
            let timed = samples
                .iter()
                .find(|s| s.kind == Kind::System && s.name == "test::timed")
                .unwrap();
            assert_eq!(timed.calls, 2);
            assert!(timed.nanos >= 4_000_000);
            assert!(timed.peak >= 2_000_000 && timed.peak <= timed.nanos);
            assert!(samples
                .iter()
                .any(|s| s.kind == Kind::Schedule && s.name.contains("Update")));
            assert!(!samples.iter().any(|s| s.name == "test::ignored"));
            assert!(!drain().iter().any(|s| s.name == "test::timed"));
        });
    }
}
