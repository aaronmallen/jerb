use serde::Deserialize;

/// The store jerb keeps its jobs in.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackendType {
  /// MySQL 8.0 or later.
  Mysql,
  /// Postgres.
  Postgres,
  /// Redis.
  Redis,
  /// SQLite.
  Sqlite,
}
