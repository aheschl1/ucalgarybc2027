use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wasmtime::{Caller, Config, Engine, Extern, Inlining, Linker, Memory, Module, StoreLimits};
use wasmtime_wasi::p1::{self, WasiP1Ctx};

/// Fuel a call starts with; `fuel_async_yield_interval` hands it out one budget at a time.
pub(crate) const RESERVE: u64 = u64::MAX / 2;

/// Fuel per millisecond of bot time: about the wasm instructions a core runs in one, so
/// `step_ms` is close to real time while every machine meters the same count.
pub const FUEL_PER_MS: u64 = 6_000_000;

/// Bot time for fuel spent.
pub fn bot_time(fuel: u64) -> Duration {
    Duration::from_nanos(fuel / (FUEL_PER_MS / 1_000_000))
}

/// wasi errno `notsup`.
const NOTSUP: i32 = 58;

/// What the snapshot and every bot share: the engine, the compiled snapshot, and the
///     functions. One per process; bots are cheap next to it.
pub struct Runtime {
    pub(crate) linker: Linker<State>,
    pub(crate) module: Module,
    pub(crate) package: PathBuf,
}

/// A bot's store data.
pub(crate) struct State {
    pub(crate) wasi: WasiP1Ctx,
    pub(crate) limits: StoreLimits,
    pub(crate) mailbox: Arc<Mutex<Mailbox>>,
    /// The reply `ucbc.take` copies out.
    reply: Vec<u8>,
    /// Fuel spent by calls before the current one, for the clock.
    pub(crate) spent: u64,
}

impl State {
    pub(crate) fn new(wasi: WasiP1Ctx, limits: StoreLimits, mailbox: Arc<Mutex<Mailbox>>) -> Self {
        Self {
            wasi,
            limits,
            mailbox,
            reply: Vec::new(),
            spent: 0,
        }
    }
}

/// Where a message waits for the driver of the call, and its reply for the guest.
#[derive(Default)]
pub(crate) struct Mailbox {
    pub(crate) message: Option<Vec<u8>>,
    pub(crate) reply: Option<Vec<u8>>,
}

impl Runtime {
    /// `package` is the installed `ucbc` directory: the snapshot is `runtime/bot.cwasm`,
    /// precompiled for this machine, and the stdlib `runtime/lib/python314.zip`.
    #[allow(unsafe_code)]
    pub fn new(package: &Path) -> wasmtime::Result<Self> {
        let engine = Engine::new(&Self::config())?;
        let path = package.join("runtime/bot.cwasm");
        // SAFETY: the file is mapped, not read, so it must not change while the module
        // lives. It is part of the installed package, written only by `make dev`.
        let module = unsafe { Module::deserialize_file(&engine, &path) }.map_err(|e| {
            e.context(format!(
                "no usable bot runtime at {}: run `make dev`",
                path.display()
            ))
        })?;
        Ok(Self {
            linker: linker(&engine)?,
            module,
            package: package.to_path_buf(),
        })
    }

    /// The engine's settings, shared with the snapshot so its precompiled module loads.
    pub fn config() -> Config {
        let mut config = Config::new();
        config
            .consume_fuel(true)
            // Bots share the snapshot's memory only when it is mapped as an image.
            .memory_guaranteed_dense_image_size(256 << 20)
            .max_wasm_stack(8 << 20)
            .async_stack_size(9 << 20)
            .cranelift_nan_canonicalization(true)
            // A few percent of wall time, no change in fuel; the snapshot compiles slower.
            .compiler_inlining(Inlining::Yes);
        config
    }
}

fn linker(engine: &Engine) -> wasmtime::Result<Linker<State>> {
    let mut linker = Linker::new(engine);
    p1::add_to_linker_sync(&mut linker, |s: &mut State| &mut s.wasi)?;
    linker.allow_shadowing(true);
    // Time is fuel spent, so a bot can see its budget and every run reads the same clock.
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "clock_time_get",
        |mut caller: Caller<'_, State>, _clock: u32, _precision: u64, out: u32| {
            let now = bot_time(caller.data().spent + (RESERVE - caller.get_fuel()?));
            let now = now.as_nanos() as u64;
            memory(&mut caller)?.write(&mut caller, out as usize, &now.to_le_bytes())?;
            wasmtime::Result::Ok(0)
        },
    )?;
    // Waiting would stall the match outside the budget: `time.sleep` raises instead.
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "poll_oneoff",
        |_: u32, _: u32, _: u32, _: u32| NOTSUP,
    )?;
    linker.func_wrap_async(
        "ucbc",
        "call",
        |mut caller: Caller<'_, State>, (ptr, len): (u32, u32)| {
            Box::new(async move {
                let mut bytes = vec![0; len as usize];
                memory(&mut caller)?.read(&caller, ptr as usize, &mut bytes)?;
                let mailbox = caller.data().mailbox.clone();
                mailbox.lock().unwrap().message = Some(bytes);
                // The driver sees the message when the call suspends here, and replies.
                let reply = std::future::poll_fn(|_| match mailbox.lock().unwrap().reply.take() {
                    Some(reply) => std::task::Poll::Ready(reply),
                    None => std::task::Poll::Pending,
                })
                .await;
                let len = u32::try_from(reply.len())?;
                caller.data_mut().reply = reply;
                Ok(len)
            })
        },
    )?;
    linker.func_wrap("ucbc", "take", |mut caller: Caller<'_, State>, ptr: u32| {
        let reply = std::mem::take(&mut caller.data_mut().reply);
        memory(&mut caller)?.write(&mut caller, ptr as usize, &reply)?;
        wasmtime::Result::Ok(())
    })?;
    Ok(linker)
}

fn memory(caller: &mut Caller<'_, State>) -> wasmtime::Result<Memory> {
    match caller.get_export("memory") {
        Some(Extern::Memory(memory)) => Ok(memory),
        _ => wasmtime::bail!("bot module exports no memory"),
    }
}
