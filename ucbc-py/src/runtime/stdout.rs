use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Stands in for `sys.stdout` and `sys.stderr` while a bot loads or steps.
#[pyclass(module = "ucbc._engine", frozen)]
#[derive(Default)]
pub struct CapturedStream {
    buf: Mutex<String>,
}

impl CapturedStream {
    pub fn take(&self) -> String {
        std::mem::take(&mut *self.buf.lock().expect("stream lock"))
    }
}

#[pymethods]
impl CapturedStream {
    #[classattr]
    #[pyo3(name = "encoding")]
    const ENCODING: &'static str = "utf-8";
    #[classattr]
    #[pyo3(name = "errors")]
    const ERRORS: &'static str = "strict";

    fn write(&self, s: &str) -> usize {
        self.buf.lock().expect("stream lock").push_str(s);
        s.len()
    }

    fn flush(&self) {}

    fn writable(&self) -> bool {
        true
    }

    fn readable(&self) -> bool {
        false
    }

    fn isatty(&self) -> bool {
        false
    }

    fn fileno(&self, py: Python<'_>) -> PyResult<()> {
        let err = PyModule::import(py, "io")?
            .getattr("UnsupportedOperation")?
            .call1(("fileno",))?;
        Err(PyErr::from_value(err))
    }
}
