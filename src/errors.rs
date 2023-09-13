use error_stack::Context;

#[derive(Debug)]
pub enum ApplicationError {
    ServerFailure,
    DatabaseConnectionFailed,
    DatabaseMigrationsFailed,
    DatabaseQueryFailed,
    DatabaseQueryError,
    UnexpectedError(&'static str),
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{self:?}"))
    }
}

impl Context for ApplicationError {}
