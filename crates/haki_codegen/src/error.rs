use thiserror::Error;

pub type CodeGenResult<T> = Result<T, CodeGenError>;

#[derive(Debug, Error)]
pub enum CodeGenError {
    #[error("unknown type `{0}` in codegen")]
    UnknownType(String),

    #[error("unknown variable `{0}` in codegen")]
    UnknownVar(String),

    #[error("unknown function `{0}` in codegen")]
    UnknownFn(String),

    #[error("LLVM builder error: {0}")]
    BuildError(String),

    #[error("IR verification failed: {0}")]
    VerificationFailed(String),
}
