mod album_union;
mod data_base;
mod entity;
mod track_union;
use crate::data_base::DB;
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db = DB::create().await?;

    db.initial_status_check(env::var("STATUS_CHECK_SONG_ID")?.as_str())
        .await
        .map_err(|error| {
            println!("Error: {}", error);
            error
        })?;

    println!("Passed status check");
    //update artist detail
    db.update_artists().await.map_err(|error| {
        println!("Error updating artists: {}", error);
        error
    })?;
    println!("Artists updated");

    //update album detail and initial round of stream updates
    db.update_albums_1().await.map_err(|error| {
        println!("Error updating albums: {}", error);
        error
    })?;
    println!("Albums updated");

    //update streams until all streams have been updated or it is within 1 hour of the end of the day
    db.update_remaining_tracks().await.map_err(|error| {
        println!("Error updating remaining tracks: {}", error);
        error
    })?;

    // db.create_artist("2uYWxilOVlUdk4oV9DvwqK").await?;
    // match db.daily_top_10().await {
    //     _ => (),
    // }
    Ok(())
}
