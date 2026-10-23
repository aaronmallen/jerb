use std::error::Error;

use jerb::Json;

#[jerb::job]
async fn send_email(Json(_): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
  Ok(())
}

async fn enqueue(client: &impl jerb::Enqueue) {
  send_email::enqueue(client, 42).await.unwrap();
}

fn main() {}
