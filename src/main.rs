use std::error::Error;
use stream_accumulator::data_base::DB;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    DB::daily_update().await?;
    Ok(())
}
