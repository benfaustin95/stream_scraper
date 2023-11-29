mod album_union;
mod data_base;
mod entity;
mod track_union;

use crate::data_base::DB;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db = DB::create().await?;
    match db.update_artists().await {
        Err(error) => println!("Error updating artists: {}", error),
        Ok(_) => match db.update_albums_1().await {
            Err(error) => println!("Error updating albums: {}", error),
            Ok(_) => println!("Albums updated"),
        },
    }
    Ok(())
}
