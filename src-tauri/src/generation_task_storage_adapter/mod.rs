//! SQLite persistence for the Generation Task aggregate and its closed outbox.

mod outbox;
mod schema;
mod task_sql;
mod translator;

pub use repository::SqliteGenerationTaskRepositoryAdapterImpl;

mod repository;

#[cfg(test)]
mod tests;
