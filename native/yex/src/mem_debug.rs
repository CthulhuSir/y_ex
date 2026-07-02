//! Runtime counters and snapshots for debugging native (NIF / yrs) memory behaviour.
//!
//! Counters are incremented at hot paths in the yex NIF layer. Pair with OS RSS and
//! `:erlang.memory(:total)` on the Elixir side to detect leaks outside the BEAM heap.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rustler::{Atom, Encoder, Env, NifResult, NifStruct, Term};
use serde::Serialize;
use yrs::updates::encoder::Encode;
use yrs::ReadTxn;

use crate::doc::NifDoc;
use crate::yinput::NifYInput;
use crate::{atoms, wrap::SliceIntoBinary};

const EVENT_COUNT: usize = 14;

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum Event {
    DocNew = 0,
    DocWithOptions = 1,
    MapSetDoc = 2,
    MapSetNull = 3,
    MapSetOther = 4,
    YoutYdocWrap = 5,
    MonitorUpdateV1 = 6,
    MonitorUpdateV2 = 7,
    MonitorSubdocs = 8,
    SubUnsubscribe = 9,
    EncodeStateAsUpdate = 10,
    ApplyUpdate = 11,
    TransactionBegin = 12,
    TransactionCommit = 13,
}

static ENABLED: AtomicBool = AtomicBool::new(true);

static COUNTERS: [AtomicU64; EVENT_COUNT] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

pub fn record(event: Event) {
    if ENABLED.load(Ordering::Relaxed) {
        COUNTERS[event as usize].fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_map_set(value: &NifYInput) {
    match value {
        NifYInput::Doc(_) => record(Event::MapSetDoc),
        NifYInput::Any(any) if any.0 == yrs::Any::Null => record(Event::MapSetNull),
        _ => record(Event::MapSetOther),
    }
}

fn counter(name: &'static str, value: u64) -> (String, u64) {
    (name.to_string(), value)
}

fn snapshot_pairs() -> Vec<(String, u64)> {
    let names = [
        "doc_new",
        "doc_with_options",
        "map_set_doc",
        "map_set_null",
        "map_set_other",
        "yout_ydoc_wrap",
        "monitor_update_v1",
        "monitor_update_v2",
        "monitor_subdocs",
        "sub_unsubscribe",
        "encode_state_as_update",
        "apply_update",
        "transaction_begin",
        "transaction_commit",
    ];

    names
        .iter()
        .enumerate()
        .map(|(idx, name)| counter(name, COUNTERS[idx].load(Ordering::Relaxed)))
        .collect()
}

#[derive(NifStruct, Serialize)]
#[module = "Yex.MemDebug.Snapshot"]
pub struct NifMemDebugSnapshot {
    pub enabled: bool,
    pub doc_new: u64,
    pub doc_with_options: u64,
    pub map_set_doc: u64,
    pub map_set_null: u64,
    pub map_set_other: u64,
    pub yout_ydoc_wrap: u64,
    pub monitor_update_v1: u64,
    pub monitor_update_v2: u64,
    pub monitor_subdocs: u64,
    pub sub_unsubscribe: u64,
    pub encode_state_as_update: u64,
    pub apply_update: u64,
    pub transaction_begin: u64,
    pub transaction_commit: u64,
    /// `map_set_doc - sub_unsubscribe` is not meaningful; use `map_set_doc - map_set_null`
    /// during unload to see net subdoc integrations vs clears.
    pub net_subdoc_integrations: i64,
}

impl NifMemDebugSnapshot {
    fn collect() -> Self {
        let doc_new = COUNTERS[Event::DocNew as usize].load(Ordering::Relaxed);
        let doc_with_options = COUNTERS[Event::DocWithOptions as usize].load(Ordering::Relaxed);
        let map_set_doc = COUNTERS[Event::MapSetDoc as usize].load(Ordering::Relaxed);
        let map_set_null = COUNTERS[Event::MapSetNull as usize].load(Ordering::Relaxed);
        let map_set_other = COUNTERS[Event::MapSetOther as usize].load(Ordering::Relaxed);
        let yout_ydoc_wrap = COUNTERS[Event::YoutYdocWrap as usize].load(Ordering::Relaxed);
        let monitor_update_v1 = COUNTERS[Event::MonitorUpdateV1 as usize].load(Ordering::Relaxed);
        let monitor_update_v2 = COUNTERS[Event::MonitorUpdateV2 as usize].load(Ordering::Relaxed);
        let monitor_subdocs = COUNTERS[Event::MonitorSubdocs as usize].load(Ordering::Relaxed);
        let sub_unsubscribe = COUNTERS[Event::SubUnsubscribe as usize].load(Ordering::Relaxed);
        let encode_state_as_update =
            COUNTERS[Event::EncodeStateAsUpdate as usize].load(Ordering::Relaxed);
        let apply_update = COUNTERS[Event::ApplyUpdate as usize].load(Ordering::Relaxed);
        let transaction_begin = COUNTERS[Event::TransactionBegin as usize].load(Ordering::Relaxed);
        let transaction_commit = COUNTERS[Event::TransactionCommit as usize].load(Ordering::Relaxed);

        NifMemDebugSnapshot {
            enabled: ENABLED.load(Ordering::Relaxed),
            doc_new,
            doc_with_options,
            map_set_doc,
            map_set_null,
            map_set_other,
            yout_ydoc_wrap,
            monitor_update_v1,
            monitor_update_v2,
            monitor_subdocs,
            sub_unsubscribe,
            encode_state_as_update,
            apply_update,
            transaction_begin,
            transaction_commit,
            net_subdoc_integrations: map_set_doc as i64 - map_set_null as i64,
        }
    }
}

#[derive(NifStruct, Serialize)]
#[module = "Yex.MemDebug.DocInfo"]
pub struct NifMemDebugDocInfo {
    pub guid: String,
    pub client_id: u64,
    pub skip_gc: bool,
    pub auto_load: bool,
    pub state_vector_bytes: u64,
    pub update_encode_bytes: u64,
    pub has_worker_pid: bool,
}

#[rustler::nif]
fn mem_debug_enable(enabled: bool) -> Atom {
    ENABLED.store(enabled, Ordering::Relaxed);
    atoms::ok()
}

#[rustler::nif]
fn mem_debug_reset() -> Atom {
    for counter in &COUNTERS {
        counter.store(0, Ordering::Relaxed);
    }
    atoms::ok()
}

#[rustler::nif]
fn mem_debug_snapshot() -> NifMemDebugSnapshot {
    NifMemDebugSnapshot::collect()
}

#[rustler::nif]
fn mem_debug_inspect_doc(doc: NifDoc) -> NifResult<NifMemDebugDocInfo> {
    doc.readonly(None, |txn| {
        let sv = txn.state_vector();
        let state_vector_bytes = sv.encode_v1().len() as u64;
        let update_encode_bytes = txn.encode_diff_v1(&yrs::StateVector::default()).len() as u64;

        Ok(NifMemDebugDocInfo {
            guid: doc.guid().to_string(),
            client_id: doc.client_id(),
            skip_gc: doc.skip_gc(),
            auto_load: doc.auto_load(),
            state_vector_bytes,
            update_encode_bytes,
            has_worker_pid: doc.worker_pid.is_some(),
        })
    })
}

#[rustler::nif]
#[allow(unused_variables)]
fn mem_debug_log<'a>(
    env: Env<'a>,
    path: String,
    phase: String,
    extra_json: Option<String>,
) -> NifResult<Atom> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let snapshot = NifMemDebugSnapshot::collect();
    let mut payload = serde_json::json!({
        "ts": ts,
        "phase": phase,
        "native": {
            "enabled": snapshot.enabled,
            "doc_new": snapshot.doc_new,
            "doc_with_options": snapshot.doc_with_options,
            "map_set_doc": snapshot.map_set_doc,
            "map_set_null": snapshot.map_set_null,
            "map_set_other": snapshot.map_set_other,
            "yout_ydoc_wrap": snapshot.yout_ydoc_wrap,
            "monitor_update_v1": snapshot.monitor_update_v1,
            "monitor_update_v2": snapshot.monitor_update_v2,
            "monitor_subdocs": snapshot.monitor_subdocs,
            "sub_unsubscribe": snapshot.sub_unsubscribe,
            "encode_state_as_update": snapshot.encode_state_as_update,
            "apply_update": snapshot.apply_update,
            "transaction_begin": snapshot.transaction_begin,
            "transaction_commit": snapshot.transaction_commit,
            "net_subdoc_integrations": snapshot.net_subdoc_integrations,
        }
    });

    if let Some(extra) = extra_json {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&extra) {
            if let Some(obj) = payload.as_object_mut() {
                if let Some(extra_obj) = value.as_object() {
                    for (k, v) in extra_obj {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    }

    let line = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }

    Ok(atoms::ok())
}

#[rustler::nif]
fn mem_debug_encode_update_size<'a>(
    env: Env<'a>,
    doc: NifDoc,
) -> NifResult<Term<'a>> {
    record(Event::EncodeStateAsUpdate);

    let bytes = doc.readonly(None, |txn| {
        Ok(txn.encode_diff_v1(&yrs::StateVector::default()))
    })?;

    Ok((
        atoms::ok(),
        bytes.len(),
        SliceIntoBinary::new(bytes.as_slice()),
    )
        .encode(env))
}
