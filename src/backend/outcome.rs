/// How the last job of a schedule ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
  /// The job failed and sits in the dead set.
  Failed,
  /// The job finished.
  Ok,
  /// The job is pending or running.
  Running,
}
