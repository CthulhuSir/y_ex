use crate::atoms;
use crate::doc::NifDoc;
use crate::event::{NifMapEvent, NifSharedTypeDeepObservable, NifSharedTypeObservable};
use crate::mem_debug::{self, Event};
use crate::shared_type::NifSharedType;
use crate::shared_type::SharedTypeId;
use crate::transaction::TransactionResource;
use crate::yinput::NifWeakPrelim;
use crate::{yinput::NifYInput, youtput::NifYOut, NifAny};
use rustler::{Atom, Env, NifResult, NifStruct, ResourceArc};
use std::collections::HashMap;
use yrs::types::ToJson;
use yrs::*;

pub type MapRefId = SharedTypeId<MapRef>;
#[derive(NifStruct)]
#[module = "Yex.Map"]
pub struct NifMap {
    doc: NifDoc,
    reference: MapRefId,
}
impl NifMap {
    pub fn new(doc: NifDoc, map: MapRef) -> Self {
        NifMap {
            doc,
            reference: MapRefId::new(map.hook()),
        }
    }
}

impl NifSharedType for NifMap {
    type RefType = MapRef;

    fn doc(&self) -> &NifDoc {
        &self.doc
    }
    fn reference(&self) -> &SharedTypeId<Self::RefType> {
        &self.reference
    }
    const DELETED_ERROR: &'static str = "Map has been deleted";
}
impl NifSharedTypeDeepObservable for NifMap {}
impl NifSharedTypeObservable for NifMap {
    type Event = NifMapEvent;
}

#[rustler::nif]
fn map_set(
    env: Env<'_>,
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
    key: &str,
    value: NifYInput,
) -> NifResult<Atom> {
    mem_debug::record_map_set(&value);
    map.mutably(env, current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        map.insert(txn, key, value);
        Ok(atoms::ok())
    })
}
#[rustler::nif]
fn map_size(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
) -> NifResult<u32> {
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        Ok(map.len(txn))
    })
}
#[rustler::nif]
fn map_get(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
    key: &str,
) -> NifResult<(Atom, NifYOut)> {
    let doc = map.doc();
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        map.get(txn, key)
            .map(|b| (atoms::ok(), NifYOut::from_native(b, doc.clone())))
            .ok_or(rustler::Error::Atom("error"))
    })
}

#[rustler::nif]
fn map_contains_key(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
    key: &str,
) -> NifResult<bool> {
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        Ok(map.contains_key(txn, key))
    })
}

#[rustler::nif]
fn map_delete(
    env: Env<'_>,
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
    key: &str,
) -> NifResult<Atom> {
    map.mutably(env, current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        map.remove(txn, key);
        Ok(atoms::ok())
    })
}
#[rustler::nif]
fn map_to_map(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
) -> NifResult<HashMap<String, NifYOut>> {
    let doc = map.doc();
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        Ok(map
            .iter(txn)
            .map(|(key, value)| (key.into(), NifYOut::from_native(value, doc.clone())))
            .collect())
    })
}
#[rustler::nif]
fn map_to_json(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
) -> NifResult<NifAny> {
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        Ok(map.to_json(txn).into())
    })
}
#[rustler::nif]
fn map_keys(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
) -> NifResult<Vec<String>> {
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        Ok(map.keys(txn).map(String::from).collect())
    })
}
#[rustler::nif]
fn map_values(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
) -> NifResult<Vec<NifYOut>> {
    let doc = map.doc();
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        // idk why values() returns Iterator<Item = Vec<Out>>
        Ok(map
            .values(txn)
            .flatten()
            .map(|v| NifYOut::from_native(v, doc.clone()))
            .collect())
    })
}

#[rustler::nif]
fn map_link(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
    key: &str,
) -> NifResult<Option<NifWeakPrelim>> {
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        let weak = map.link(txn, key).map(|w| NifWeakPrelim::new(w.upcast()));
        Ok(weak)
    })
}

#[rustler::nif]
fn map_count_embedded_subdocs(
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
) -> NifResult<u64> {
    map.readonly(current_transaction, |txn| {
        let map = map.get_ref(txn)?;
        Ok(map
            .iter(txn)
            .filter(|(_, value)| matches!(value, Out::YDoc(_)))
            .count() as u64)
    })
}

#[rustler::nif]
fn map_destroy_all_embedded_subdocs(
    env: Env<'_>,
    map: NifMap,
    current_transaction: Option<ResourceArc<TransactionResource>>,
) -> NifResult<u64> {
    mem_debug::record(Event::MapDestroySubdocs);
    let parent = map.doc().clone();
    map.mutably(env, current_transaction, |txn| {
        let map_ref = map.get_ref(txn)?;
        let keys: Vec<String> = map_ref.keys(txn).map(String::from).collect();
        let mut destroyed = 0u64;

        for key in keys {
            if let Some(Out::YDoc(subdoc)) = map_ref.get(txn, key.as_str()) {
                let guid = subdoc.guid().to_string();
                subdoc.destroy(txn);
                parent.evict_subdoc_cache(&guid);
                map_ref.insert(txn, key.as_str(), NifYInput::Any(Any::Null.into()));
                destroyed += 1;
            }
        }

        if destroyed > 0 {
            txn.gc(None);
            mem_debug::record(Event::DocGc);
            parent.clear_subdoc_cache();
        }

        Ok(destroyed)
    })
}
