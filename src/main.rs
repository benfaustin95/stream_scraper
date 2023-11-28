mod DB;
mod album_union;
mod entity;
mod track_union;

use crate::album_union::AlbumUnion;
use crate::track_union::GetUnion;
use chrono::{Datelike, TimeZone};
use entity::{prelude::*, *};
use futures::StreamExt;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db = DB::create().await?;
    // println!("{}", get_date());
    // db.update_artists().await?;let db = DB::create().await?;
    let artists = Artist::find().all(&db.db).await?;
    let artist_ids: Vec<&str> = artists.iter().map(|x| x.id.as_str()).collect();
    let artist_map = artist_ids.into_iter().collect::<HashSet<&str>>();
    let album_union = AlbumUnion::get_union("1o59UpKw81iHR0HPiSkJR0").await?;
    album_union.update(&db, &artist_map).await?;
    Ok(())
}
