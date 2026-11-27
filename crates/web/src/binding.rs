//! The existing exact-origin IndexedDB half, not an account bearer.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use futures::channel::oneshot;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{IdbDatabase, IdbRequest, IdbTransactionMode};

thread_local! {
    static CHANGES: Option<web_sys::BroadcastChannel> = web_sys::BroadcastChannel::new("boardcraft-session-changed").ok();
}

pub(super) fn changes() -> Option<web_sys::BroadcastChannel> {
    CHANGES.with(Clone::clone)
}

struct Database(IdbDatabase);

impl Drop for Database {
    fn drop(&mut self) {
        self.0.close();
    }
}

struct PendingRequest {
    request: IdbRequest,
    _callback: Completion,
}

impl Drop for PendingRequest {
    fn drop(&mut self) {
        self.request.set_onsuccess(None);
        self.request.set_onerror(None);
        if let Some(opening) = self.request.dyn_ref::<web_sys::IdbOpenDbRequest>() {
            opening.set_onblocked(None);
        }
    }
}

struct Upgrade {
    request: web_sys::IdbOpenDbRequest,
    _callback: Closure<dyn FnMut()>,
}

impl Drop for Upgrade {
    fn drop(&mut self) {
        self.request.set_onupgradeneeded(None);
    }
}

struct PendingTransaction {
    transaction: web_sys::IdbTransaction,
    request: IdbRequest,
    _completion: Completion,
    _update: Closure<dyn FnMut()>,
}

impl Drop for PendingTransaction {
    fn drop(&mut self) {
        self.transaction.set_oncomplete(None);
        self.transaction.set_onabort(None);
        self.request.set_onsuccess(None);
    }
}

type Completion = Closure<dyn FnMut(web_sys::Event)>;

fn completion() -> (oneshot::Receiver<bool>, Completion) {
    let (sender, receiver) = oneshot::channel();
    let sender = RefCell::new(Some(sender));
    let callback = Closure::new(move |event: web_sys::Event| {
        if let Some(sender) = sender.borrow_mut().take() {
            let _ = sender.send(matches!(event.type_().as_str(), "success" | "complete"));
        }
    });
    (receiver, callback)
}

async fn request(request: &IdbRequest) -> Result<JsValue, ()> {
    let (receiver, callback) = completion();
    request.set_onsuccess(Some(callback.as_ref().unchecked_ref()));
    request.set_onerror(Some(callback.as_ref().unchecked_ref()));
    if let Some(opening) = request.dyn_ref::<web_sys::IdbOpenDbRequest>() {
        opening.set_onblocked(Some(callback.as_ref().unchecked_ref()));
    }
    let _pending = PendingRequest {
        request: request.clone(),
        _callback: callback,
    };
    if receiver.await != Ok(true) {
        return Err(());
    }
    request.result().map_err(|_| ())
}

async fn open() -> Result<Database, ()> {
    let factory = web_sys::window()
        .ok_or(())?
        .indexed_db()
        .map_err(|_| ())?
        .ok_or(())?;
    let opening = factory.open_with_u32("boardcraft-web", 1).map_err(|_| ())?;
    let upgraded = opening.clone();
    let upgrade = Closure::<dyn FnMut()>::new(move || {
        if let Ok(value) = upgraded.result()
            && let Ok(db) = value.dyn_into::<IdbDatabase>()
        {
            let _ = db.create_object_store("session");
        }
    });
    opening.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    let _upgrade = Upgrade {
        request: opening.clone(),
        _callback: upgrade,
    };
    let result = request(&opening).await;
    result?.dyn_into().map(Database).map_err(|_| ())
}

pub(super) async fn read() -> Result<Option<String>, ()> {
    let db = open().await?;
    let tx = db.0.transaction_with_str("session").map_err(|_| ())?;
    let value = request(
        &tx.object_store("session")
            .map_err(|_| ())?
            .get(&"binding".into())
            .map_err(|_| ())?,
    )
    .await?;
    if value.is_undefined() {
        Ok(None)
    } else {
        value.as_string().map(Some).ok_or(())
    }
}

pub(super) async fn save(binding: &str) -> Result<(), ()> {
    let db = open().await?;
    let tx =
        db.0.transaction_with_str_and_mode("session", IdbTransactionMode::Readwrite)
            .map_err(|_| ())?;
    let (receiver, callback) = completion();
    tx.set_oncomplete(Some(callback.as_ref().unchecked_ref()));
    tx.set_onabort(Some(callback.as_ref().unchecked_ref()));
    let put = tx
        .object_store("session")
        .map_err(|_| ())?
        .put_with_key(&binding.into(), &"binding".into())
        .map_err(|_| ())?;
    let _pending = PendingTransaction {
        transaction: tx,
        request: put,
        _completion: callback,
        _update: Closure::new(|| {}),
    };
    if receiver.await == Ok(true) {
        broadcast();
        Ok(())
    } else {
        Err(())
    }
}

fn broadcast() {
    if let Some(channel) = changes() {
        let _ = channel.post_message(&JsValue::NULL);
    }
}

/// Compare and remove in one IndexedDB transaction: a delayed rejection must
/// never forget a replacement login from another tab.
pub(super) async fn forget(expected: &str) -> Result<(), ()> {
    let db = open().await?;
    let tx =
        db.0.transaction_with_str_and_mode("session", IdbTransactionMode::Readwrite)
            .map_err(|_| ())?;
    let store = tx.object_store("session").map_err(|_| ())?;
    let get = store.get(&"binding".into()).map_err(|_| ())?;
    let read = get.clone();
    let transaction = tx.clone();
    let changed = Rc::new(Cell::new(false));
    let changed_on_read = changed.clone();
    let expected = expected.to_owned();
    let update = Closure::<dyn FnMut()>::new(move || {
        if read.result().ok().and_then(|v| v.as_string()).as_deref() == Some(&expected) {
            if store.delete(&"binding".into()).is_err() {
                let _ = transaction.abort();
            } else {
                changed_on_read.set(true);
            }
        }
    });
    let (receiver, callback) = completion();
    tx.set_oncomplete(Some(callback.as_ref().unchecked_ref()));
    tx.set_onabort(Some(callback.as_ref().unchecked_ref()));
    get.set_onsuccess(Some(update.as_ref().unchecked_ref()));
    let _pending = PendingTransaction {
        transaction: tx.clone(),
        request: get,
        _completion: callback,
        _update: update,
    };
    if receiver.await != Ok(true) {
        return Err(());
    }
    if changed.get() {
        broadcast();
    }
    Ok(())
}
