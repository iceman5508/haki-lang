use thiserror::Error;

pub type WasmResult<T> = Result<T, WasmError>;

#[derive(Debug, Error)]
pub enum WasmError {
    #[error("unsupported type in Wasm backend: {0}")]
    UnsupportedType(String),

    #[error("unsupported instruction in Wasm backend: {0}")]
    UnsupportedOp(String),

    #[error("unknown variable `{0}` in Wasm emitter")]
    UnknownVar(String),

    #[error("unknown function `{0}` in Wasm emitter")]
    UnknownFn(String),
}
