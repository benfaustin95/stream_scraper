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
use serde::Deserialize;
use crate::track_union::{get_union, TrackUnion};

async fn getDB() -> Result<DatabaseConnection, DbErr> {
    dotenv().ok();
    let db_url = env::var("DATABASE_URL").unwrap();
    Ok(Database::connect(db_url).await?)
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
    let track: track::Model = Track::find_by_id(id).one(&db).await?.unwrap();
    let ds = track
        .find_related(DailyStreams)
        .order_by_desc(daily_streams::Column::Date)
        .limit(3)
        .all(&db)
        .await?;
    let track = get_union::<TrackUnion>(
        "https://2p3vesqneoheqyxagoxh5wrtay0nednp.lambda-url.us-west-2.on.aws/",
        id).await?;
    println!("{:?}", track);
    Ok(true)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    match block_on(initial_status_check("7KR99ZBAg8oiupNIinrgRF")) {
        Ok(value) => println!("{:?}", value),
        Err(error) => println!("{}", error),
    }
    Ok(())
}
