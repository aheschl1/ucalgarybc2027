//! `ucbc-dev snapshot`: runs the guest's `init`, which starts CPython and imports
//! `ucbc._guest`, and writes the initialized instance out precompiled, as the module
//! every bot is created from.

use std::path::Path;

use wasmtime::{Engine, Instance, Linker, Module, Store, Val, bail};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder};
use wasmtime_wizer::{InstanceState, SnapshotVal, ValType, Wizer};

use crate::Runtime;

/// `guest` is the linked interpreter, `package` the `ucbc` directory with the stdlib zip
/// in `runtime/lib`, `target` the triple to compile for when it is not this machine's.
/// The mounts are the ones a bot gets, in the same order: wasi-libc records them during
/// `init`, so the snapshot expects them at the same descriptors. Nothing reads `/bot`
/// yet, so it can be any directory.
pub fn snapshot(
    guest: &Path,
    package: &Path,
    target: Option<&str>,
    out: &Path,
) -> wasmtime::Result<()> {
    let engine = Engine::default();
    let mut linker = Linker::new(&engine);
    p1::add_to_linker_sync(&mut linker, |wasi: &mut WasiP1Ctx| wasi)?;
    linker.func_wrap("ucbc", "call", |_: u32, _: u32| -> wasmtime::Result<u32> {
        bail!("ucbc.call during init")
    })?;
    linker.func_wrap("ucbc", "take", |_: u32| -> wasmtime::Result<()> {
        bail!("ucbc.take during init")
    })?;
    let wasi = WasiCtxBuilder::new()
        .inherit_stderr()
        .preopened_dir(package.join("runtime/lib"), "/lib", FsPerms::ReadOnly)?
        .preopened_dir(package, "/ucbc", FsPerms::ReadOnly)?
        .preopened_dir(package, "/bot", FsPerms::ReadOnly)?
        .build_p1();
    let mut store = Store::new(&engine, wasi);

    let wizer = Wizer::new();
    let guest = std::fs::read(guest)?;
    let (cx, instrumented) = wizer.instrument(&guest)?;
    let module = Module::new(&engine, &instrumented)?;
    let snapshot = pollster::block_on(async {
        let instance = linker.instantiate_async(&mut store, &module).await?;
        // A reactor: its C constructors, then Python.
        for name in ["_initialize", "init"] {
            let func = instance.get_typed_func::<(), ()>(&mut store, name)?;
            func.call_async(&mut store, ()).await?;
        }
        wizer.snapshot(&cx, &mut Started { store, instance }).await
    })?;
    let mut config = Runtime::config();
    if let Some(target) = target {
        config.target(target)?;
    }
    std::fs::write(out, Engine::new(&config)?.precompile_module(&snapshot)?)?;
    Ok(())
}

/// The started instance, as wizer reads it.
struct Started {
    store: Store<WasiP1Ctx>,
    instance: Instance,
}

impl InstanceState for Started {
    async fn global_get(&mut self, name: &str, _: ValType) -> SnapshotVal {
        let global = self.instance.get_global(&mut self.store, name).unwrap();
        match global.get(&mut self.store) {
            Val::I32(x) => SnapshotVal::I32(x),
            Val::I64(x) => SnapshotVal::I64(x),
            Val::F32(x) => SnapshotVal::F32(x),
            Val::F64(x) => SnapshotVal::F64(x),
            Val::V128(x) => SnapshotVal::V128(x.as_u128()),
            other => panic!("global {name} has an unsupported type: {other:?}"),
        }
    }

    async fn memory_contents(&mut self, name: &str, contents: impl FnOnce(&[u8]) + Send) {
        let memory = self.instance.get_memory(&mut self.store, name).unwrap();
        contents(memory.data(&self.store))
    }
}
