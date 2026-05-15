use rusqlite::types::Type;

pub(crate) fn conversion_err<E>(err: E) -> rusqlite::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    // Column index is not surfaced in our error path; 0 is a conventional placeholder.
    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err))
}
