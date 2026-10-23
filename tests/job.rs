use std::error::Error;

#[jerb::job]
async fn perform() -> Result<(), Box<dyn Error + Send + Sync>> {
  Ok(())
}

mod job {
  use super::*;

  #[tokio::test]
  async fn it_leaves_the_function_as_written() {
    assert!(perform().await.is_ok());
  }

  #[test]
  fn it_rejects_enqueue_with_the_wrong_args() {
    trybuild::TestCases::new().compile_fail("tests/ui/enqueue_with_the_wrong_args.rs");
  }
}
