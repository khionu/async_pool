use std::{
    ops::{Deref, DerefMut},
    sync::RwLock,
};

use futures::channel::mpsc::{
    unbounded, UnboundedReceiver, UnboundedSender,
};
use futures::SinkExt;
use futures::stream::StreamExt;

/// A collection for asynchronously reserving a resource from a pool
pub struct AsyncPool<T: Send + 'static> {
    /// The inbox, where dropped guards send to.
    rx: RwLock<UnboundedReceiver<T>>,
    /// The sender to clone for new guards.
    tx: RwLock<UnboundedSender<T>>,
}

// TODO: Avoid move-by-value
/// The guard on a resource's borrow. Returns the value to the `AsyncPool` on drop.
pub struct AsyncPoolGuard<T: Send + 'static> {
    inner: Option<T>,
    tx: Box<dyn FnMut(T)>,
}

impl<T: Send + 'static> AsyncPool<T> {
    /// Create a new `AsyncPool`.
    pub fn new() -> Self {
        let (tx, rx) = unbounded();

        Self {
            rx: RwLock::new(rx),
            tx: RwLock::new(tx),
        }
    }

    /// Create a new `AsyncPool` using an initial set of resources
    pub async fn new_with(mut initial_resources: Vec<T>) -> Self {
        let new = Self::new();

        for i in initial_resources.drain(..) {
            new.add(i).await
        }

        new
    }

    /// Await the next available resource
    pub async fn rsvp(&self) -> AsyncPoolGuard<T> {
        let tx = self.tx.read().expect("Poisoned sender").clone();

        let mut rx = self.rx.write().expect("Poisoned receiver");

        AsyncPoolGuard {
            inner: Some(rx.next().await.unwrap()),
            tx: Box::new(move |x| tx.unbounded_send(x).expect("Pool was dropped before guard")),
        }
    }

    /// Add `item` to the current `AsyncPool`.
    pub async fn add(&self, item: T) {
        // This unwrap is safe because we have a reference to the owner of the receiver
        self.tx.write().expect("Poisoned sender").send(item).await.unwrap()
    }
}

impl<T: Send + 'static> Drop for AsyncPoolGuard<T> {
    fn drop(&mut self) {
        if let Some(i) = self.inner.take() {
            (self.tx)(i);
        }
    }
}

impl<T: Send + 'static> Deref for AsyncPoolGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
            .expect("Inner value dropped while Guard was active")
    }
}

impl<T: Send + 'static> DerefMut for AsyncPoolGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut()
            .expect("Inner value dropped while Guard was active")
    }
}

#[cfg(test)]
mod tests {
    // TODO: Tests
}
