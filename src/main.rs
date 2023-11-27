mod entity;
mod track_union;

use dotenv::dotenv;
use entity::{prelude::*, *};
use futures::executor::block_on;
use futures::{StreamExt, TryFutureExt};
use sea_orm::*;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use async_trait::async_trait;
use serde::Deserialize;
use crate::track_union::{TrackUnion};

async fn getDB() -> Result<DatabaseConnection, DbErr> {

}

struct DB {
    db: DatabaseConnection
}

#[async_trait]
impl DB {
    async fn create() -> Result<Self, DbErr> {
        dotenv().ok();
        let db_url = env::var("DATABASE_URL").unwrap();
        Ok(Self {
            db: Database::connect(db_url).await?,
        })
    }
}
pub async fn get_data<T: for <'a> Deserialize<'a>>(url: &str, body: HashMap<&str, &str>) -> Result<T, reqwest::Error> {
    let client = reqwest::Client::new();
    client.get(url)
        .json(&body)
        .send()
        .await?
        .json::<T>()
        .await
}

pub async fn initial_status_check(id: &str) -> Result<bool, Box<dyn Error>> {
    let db = getDB().await?;
    let current_track= get_track_by_id(id).await?;
    let updated_track = TrackUnion::get_union(id).await?;
    current_track.compare_streams(updated_track.playcount).await?
}

async fn get_track_by_id(id: &str) -> Result<track::Model, Box<dyn Error>> {
    let track: track::Model = Track::find_by_id(id).one(&db).await?.unwrap();
    Ok(track)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    match block_on(initial_status_check("7KR99ZBAg8oiupNIinrgRF")) {
        Ok(value) => println!("{:?}", value),
        Err(error) => println!("{}", error),
    }
    Ok(())
}
