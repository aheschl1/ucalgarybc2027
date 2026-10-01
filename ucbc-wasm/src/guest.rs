use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use wasmtime::{Instance, Store, StoreLimitsBuilder, WasmParams};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder};

use crate::runtime::{Mailbox, RESERVE, Runtime, State};

/// A call in progress, owning the store until it returns.
type Call = Pin<Box<dyn Future<Output = (Store<State>, wasmtime::Result<()>)> + Send>>;

/// One bot: its own instance of the snapshot, `/bot` and `/cache` mounted read-only.
pub struct Guest {
    /// Absent while a call is suspended; the call holds it.
    store: Option<Store<State>>,
    suspended: Option<Call>,
    instance: Instance,
    mailbox: Arc<Mutex<Mailbox>>,
    /// The store's fuel when this turn began, and the turn's budget.
    turn_start: u64,
    budget: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Returned,
    /// Out of fuel: suspended, and resumed by the next `load` or `step`.
    OutOfFuel,
}

#[derive(Debug)]
pub struct Run {
    pub outcome: Outcome,
    /// Fuel used this turn.
    pub fuel: u64,
}

impl Guest {
    /// `cache` holds the bytecode [`Guest::compile`] wrote for `bot_dir`; `seed` drives
    /// everything random the bot sees; `memory_bytes` caps its memory, the interpreter's
    /// included.
    pub fn new(
        runtime: &Runtime,
        bot_dir: &Path,
        cache: &Path,
        seed: u64,
        memory_bytes: u64,
    ) -> wasmtime::Result<Self> {
        Self::instantiate(
            runtime,
            bot_dir,
            cache,
            FsPerms::ReadOnly,
            seed,
            memory_bytes,
        )
    }

    /// Compiles every module under `bot_dir` to bytecode in `cache`, for the bots created
    /// with it to load: once per team, on no bot's budget.
    pub fn compile(
        runtime: &Runtime,
        bot_dir: &Path,
        cache: &Path,
        memory_bytes: u64,
        fuel: u64,
    ) -> wasmtime::Result<Outcome> {
        let mut guest =
            Self::instantiate(runtime, bot_dir, cache, FsPerms::ReadWrite, 0, memory_bytes)?;
        let run = guest.run("compile", (), fuel, |_| Vec::new())?;
        Ok(run.outcome)
    }

    fn instantiate(
        runtime: &Runtime,
        bot_dir: &Path,
        cache: &Path,
        cache_perms: FsPerms,
        seed: u64,
        memory_bytes: u64,
    ) -> wasmtime::Result<Self> {
        // The mounts `ucbc-dev snapshot` made, in the same order.
        let package = &runtime.package;
        let wasi = WasiCtxBuilder::new()
            .preopened_dir(package.join("runtime/lib"), "/lib", FsPerms::ReadOnly)?
            .preopened_dir(package, "/ucbc", FsPerms::ReadOnly)?
            .preopened_dir(bot_dir, "/bot", FsPerms::ReadOnly)?
            .preopened_dir(cache, "/cache", cache_perms)?
            .secure_random(ChaCha20Rng::seed_from_u64(seed))
            .insecure_random(ChaCha20Rng::seed_from_u64(seed))
            .insecure_random_seed(u128::from(seed))
            .build_p1();
        let limits = StoreLimitsBuilder::new()
            .memory_size(memory_bytes as usize)
            .build();
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let mut store = Store::new(
            runtime.linker.engine(),
            State::new(wasi, limits, mailbox.clone()),
        );
        store.limiter(|s| &mut s.limits);
        store.set_fuel(RESERVE)?;
        // Nothing in instantiation waits.
        let instance = pollster::block_on(
            runtime
                .linker
                .instantiate_async(&mut store, &runtime.module),
        )?;
        Ok(Self {
            store: Some(store),
            suspended: None,
            instance,
            mailbox,
            turn_start: RESERVE,
            budget: 0,
        })
    }

    /// Imports the bot's `main.py`, or resumes a suspended call.
    pub fn load(
        &mut self,
        fuel: u64,
        on_message: impl FnMut(Vec<u8>) -> Vec<u8>,
    ) -> wasmtime::Result<Run> {
        self.run("load", (), fuel, on_message)
    }

    /// Runs one step, or resumes a suspended call instead.
    pub fn step(
        &mut self,
        set_index: u32,
        tick: u32,
        fuel: u64,
        on_message: impl FnMut(Vec<u8>) -> Vec<u8>,
    ) -> wasmtime::Result<Run> {
        self.run("step", (set_index as i32, tick as i32), fuel, on_message)
    }

    /// The bot's memory size, unless a call is suspended.
    pub fn memory_bytes(&mut self) -> Option<u64> {
        let store = self.store.as_mut()?;
        let memory = self.instance.get_memory(&mut *store, "memory")?;
        Some(memory.data_size(&*store) as u64)
    }

    fn run<P: WasmParams + Send + Sync + 'static>(
        &mut self,
        export: &str,
        params: P,
        fuel: u64,
        on_message: impl FnMut(Vec<u8>) -> Vec<u8>,
    ) -> wasmtime::Result<Run> {
        let call = match self.suspended.take() {
            Some(call) => call,
            None => self.start(export, params, fuel)?,
        };
        self.drive(call, on_message)
    }

    fn start<P: WasmParams + Send + Sync + 'static>(
        &mut self,
        export: &str,
        params: P,
        fuel: u64,
    ) -> wasmtime::Result<Call> {
        let mut store = self.store.take().expect("no call in progress");
        // A missing export is a broken snapshot; the store is not worth keeping.
        let func = self.instance.get_typed_func::<P, ()>(&mut store, export)?;
        store.data_mut().spent += RESERVE - store.get_fuel()?;
        store.fuel_async_yield_interval(Some(fuel))?;
        store.set_fuel(RESERVE)?;
        self.turn_start = RESERVE;
        self.budget = fuel;
        Ok(Box::pin(async move {
            let result = func.call_async(&mut store, params).await;
            (store, result)
        }))
    }

    /// Polls the call, answering its messages, until it returns or runs out of fuel.
    fn drive(
        &mut self,
        mut call: Call,
        mut on_message: impl FnMut(Vec<u8>) -> Vec<u8>,
    ) -> wasmtime::Result<Run> {
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            match call.as_mut().poll(&mut cx) {
                Poll::Ready((store, result)) => {
                    let fuel = self.turn_start - store.get_fuel()?;
                    self.store = Some(store);
                    result?;
                    return Ok(Run {
                        outcome: Outcome::Returned,
                        fuel,
                    });
                }
                Poll::Pending => {
                    let message = self.mailbox.lock().unwrap().message.take();
                    match message {
                        Some(message) => {
                            let reply = on_message(message);
                            self.mailbox.lock().unwrap().reply = Some(reply);
                        }
                        None => {
                            self.suspended = Some(call);
                            self.turn_start -= self.budget;
                            return Ok(Run {
                                outcome: Outcome::OutOfFuel,
                                fuel: self.budget,
                            });
                        }
                    }
                }
            }
        }
    }
}
