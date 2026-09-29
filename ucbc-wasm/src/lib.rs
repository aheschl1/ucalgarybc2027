//! Python bots as WebAssembly. Each bot is a wasmtime instance of CPython built for
//! WASI (`guest/`), created from a snapshot taken once the interpreter has started
//! (`ucbc-dev snapshot`, [`snapshot`] with the `snapshot` feature), so bots share its
//! memory until they write to it.
//!
//! A bot runs on fuel: a budget of wasm instructions per call, the same on every
//! machine. A call that runs out is suspended and the next call resumes it. The guest
//! (`ucbc._guest`) reaches the engine only by message: JSON out, a JSON reply back,
//! answered by whoever drives the call.

mod guest;
mod runtime;
#[cfg(feature = "snapshot")]
mod snapshot;

pub use guest::{Guest, Outcome, Run};
pub use runtime::Runtime;
#[cfg(feature = "snapshot")]
pub use snapshot::snapshot;
