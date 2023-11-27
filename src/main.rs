mod entity;
mod track_union;

use crate::track_union::{GetUnion, TrackUnion};
use dotenv::dotenv;
use entity::{prelude::*, *};
use futures::StreamExt;
use sea_orm::sea_query::IndexType::Hash;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::Value::Null;
use std::collections::HashMap;
use std::env;
use std::error::Error;

struct DB {
    db: DatabaseConnection,
}

impl DB {
    async fn create() -> Result<Self, DbErr> {
        dotenv().ok();
        let db_url = env::var("DATABASE_URL").unwrap();
        Ok(Self {
            db: Database::connect(db_url).await?,
        })
    }
    pub async fn get_track_by_id(&self, id: &str) -> Result<track::Model, Box<dyn Error>> {
        let track: track::Model = Track::find_by_id(id).one(&self.db).await?.unwrap();
        Ok(track)
    }

    pub async fn compare_streams(&self, id: &str, playcount: u64) -> Result<bool, Box<dyn Error>> {
        let track = self.get_track_by_id(id).await?;
        let ds: Vec<daily_streams::Model> = track
            .find_related(DailyStreams)
            .order_by_desc(daily_streams::Column::Date)
            .limit(3)
            .all(&self.db)
            .await?;
        if ds.is_empty()
            || ds[0].streams as u64 != playcount
            || (ds.len() >= 2 && ds[0].streams - ds[1].streams <= 100)
        {
            return Ok(true);
        }
        return Ok(false);
    }

    pub async fn initial_status_check(&self, id: &str) -> Result<bool, Box<dyn Error>> {
        let updated_track = TrackUnion::get_union(id).await?;
        self.compare_streams(id, updated_track.playcount).await
    }

    pub async fn update_artists(&self) -> Result<bool, Box<dyn Error>> {
        let artists = Artist::find().all(&self.db).await?;
        self.update_artist_detail(&artists).await?;
        Ok(true)
    }

    async fn update_artist_detail(
        &self,
        artists: &Vec<artist::Model>,
    ) -> Result<bool, Box<dyn Error>> {
        let access_token = get_spotify_access_token().await?;
        let ids: Vec<&str> = artists.iter().map(|x| x.id.as_str()).collect();
        let id_string = ids.join("%2C");
        let temp = reqwest::Client::new()
            .get(format!(
                "{}/{}?ids={}",
                "https://api.spotify.com/v1", "artists", id_string
            ))
            .header(
                "Authorization",
                format!("Bearer {}", access_token.access_token),
            )
            .send()
            .await?
            .json::<ArtistsAPI>()
            .await?;

        for artist in temp.artists {
            let mut active: artist::ActiveModel = Artist::find_by_id(artist.id)
                .one(&self.db)
                .await?
                .unwrap()
                .into();
            active.name = Set(artist.name.to_owned());
            let mut images = Vec::new();
            for image in artist.images {
                images.push(serde_json::to_string(&image).unwrap());
            }
            active.images = Set(images);
            let update = active.update(&self.db).await?;
            println!("{:?}", update);
        }

        Ok(true)
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct Image {
    url: String,
    height: u32,
    width: u32,
}
#[derive(Deserialize, Serialize, Debug)]
struct ArtistAPI {
    id: String,
    name: String,
    images: Vec<Image>,
}

#[derive(Deserialize, Serialize, Debug)]
struct ArtistsAPI {
    artists: Vec<ArtistAPI>,
}

#[derive(Deserialize, Serialize, Debug)]
struct AccessToken {
    access_token: String,
}

async fn get_spotify_access_token() -> Result<AccessToken, reqwest::Error> {
    let client = reqwest::Client::new();
    client.post("https://accounts.spotify.com/api/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials&client_id=079e4a08d2d242139097547ece7b354b&client_secret=3480073d1d8746cf9477f8469581d935")
        .send()
        .await?
        .json::<AccessToken>()
        .await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db = DB::create().await?;
    db.update_artists().await?;
    // match block_on(initial_status_check("7KR99ZBAg8oiupNIinrgRF")) {
    //     Ok(value) => println!("{:?}", value),
    //     Err(error) => println!("{}", error),
    // }
    Ok(())
}
