use std::error::Error;
use stream_accumulator::modules;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    match modules::data_base::DB::daily_update().await {
        Err(error) => println!("Error performing update: {}", error),
        Ok(value) => println!("Update duration: {}", value),
    }
    Ok(())
}
