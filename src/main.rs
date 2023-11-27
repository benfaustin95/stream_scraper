mod album_union;
mod entity;
mod track_union;

use crate::album_union::AlbumUnion;
use crate::track_union::{get_data, GetUnion, TrackUnion};
use chrono::{DateTime, Datelike, Days, Local, TimeZone, Utc};
use dotenv::dotenv;
use entity::{prelude::*, *};
use futures::{future, stream, StreamExt};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
        let artist_ids: Vec<&str> = artists.iter().map(|x| x.id.as_str()).collect();
        // self.update_artist_detail(&artist_ids).await?;
        let ids = DB::get_album_ids(&artist_ids).await?;
        let artist_map = artist_ids.into_iter().collect::<HashSet<&str>>();
        DB::update_albums(ids, artist_map).await?;
        Ok(true)
    }

    async fn update_albums(
        albums: HashSet<String>,
        artists: HashSet<&str>,
    ) -> Result<bool, Box<dyn Error>> {
        let chunk = 350;
        let response_bodies = stream::iter(albums)
            .map(|id| async move { DB::update_album(id).await })
            .buffer_unordered(chunk);
        Ok(true)
    }

    async fn update_album(
        id: String,
        // _artists: &HashSet<&str>,
    ) -> Result<AlbumUnion, reqwest::Error> {
        let album_union = AlbumUnion::get_union(id.as_str()).await?;
        let db = DB::create().await?;
    }
    async fn get_album_ids(artist: &Vec<&str>) -> Result<HashSet<String>, reqwest::Error> {
        let response_bodies: Vec<Result<Vec<String>, reqwest::Error>> =
            future::join_all(artist.iter().map(|artist_id| {
                let body = vec![("artistID", *artist_id)]
                    .into_iter()
                    .collect::<HashMap<&str, &str>>();
                println!("artist request: {:?}", body);
                async move {
                    get_data::<Vec<String>>(
                        "https://sc62bwganjfc5tklg2npefyfwy0nvcys.lambda-url.us-west-2.on.aws/",
                        body,
                    )
                    .await
                }
            }))
            .await;

        let mut ids = Vec::new();
        for response in response_bodies {
            let response_ids = response.unwrap();
            ids.push(response_ids);
        }
        let flat_ids = ids.into_iter().flatten().collect::<HashSet<String>>();
        Ok(flat_ids)
    }

    async fn update_artist_detail(&self, artists: &Vec<&str>) -> Result<bool, Box<dyn Error>> {
        let access_token = get_spotify_access_token().await?;
        let temp = reqwest::Client::new()
            .get(format!(
                "{}/{}?ids={}",
                "https://api.spotify.com/v1",
                "artists",
                artists.join("%2C")
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
            let updated = active.update(&self.db).await?;
            follower_instance::ActiveModel {
                artist_id: Set(updated.id.to_owned()),
                count: Set(artist.followers.total as i32),
                date: Set(get_date().date_naive()),
            }
            .insert(&self.db)
            .await?;
        }
        Ok(true)
    }
}

pub fn get_date() -> DateTime<Utc> {
    let date = Local::now().checked_sub_days(Days::new(1)).unwrap();
    Utc.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .unwrap()
}
#[derive(Deserialize, Serialize, Debug)]
struct FollowersAPI {
    href: Option<String>,
    total: u64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Image {
    url: String,
    height: u32,
    width: u32,
}
#[derive(Deserialize, Serialize, Debug)]
struct ArtistAPI {
    id: String,
    name: String,
    images: Vec<Image>,
    followers: FollowersAPI,
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
    // println!("{}", get_date());
    // db.update_artists().await?;
    DB::update_album("1fc8tPf36cZhNYpNFrWh7o".to_string()).await?;
    Ok(())
}
