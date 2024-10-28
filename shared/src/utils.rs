use std::error::Error;

pub type BoxError = Box<dyn Error + Send + Sync>;

pub type BoxResult<T> = Result<T, BoxError>;
