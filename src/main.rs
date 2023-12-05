use std::error::Error;
use stream_accumulator::modules;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    modules::data_base::DB::daily_update().await?;
    Ok(())
}
