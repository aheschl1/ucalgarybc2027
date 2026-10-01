//! Python bots as WebAssembly. Each bot is a wasmtime instance of CPython built for
//! WASI (`guest/`), created from a snapshot taken once the interpreter has started
//! (`ucbc-dev snapshot`, [`snapshot`] with the `snapshot` feature), so bots share its
//! memory until they write to it.
//!
//! A bot runs on fuel: a budget of wasm instructions per call, the same on every
//! machine. A call that runs out is suspended and the next call resumes it. The guest
//! (`ucbc._guest`) reaches the engine only by message: bytes out, a reply back, answered
//! by whoever drives the call, in whatever encoding the two agree on (`ucbc-cli`: pickle).

mod guest;
mod runtime;
#[cfg(feature = "snapshot")]
mod snapshot;

pub use guest::{Guest, Outcome, Run};
pub use runtime::{FUEL_PER_MS, Runtime, bot_time};
#[cfg(feature = "snapshot")]
pub use snapshot::snapshot;
