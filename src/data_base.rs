use crate::album_union::{AlbumUnion, ExtractedColors};
use crate::entity::{prelude::*, *};
use crate::track_union::{get_artist_data, get_data, GetUnion, Image, TrackUnion};
use async_recursion::async_recursion;
use chrono::{DateTime, Datelike, Days, Local, TimeZone, Utc};
use futures::{future, stream, StreamExt};
use sea_orm::prelude::Date;
use sea_orm::{
    sea_query::{OnConflict, Query},
    ColumnTrait, Condition, Database, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    ModelTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, env, error::Error};
use tokio::time::{sleep, Duration};

pub struct DB {
    pub db: DatabaseConnection,
}

impl DB {
    pub async fn create() -> Result<Self, DbErr> {
        dotenv::dotenv().ok();
        let db_url = env::var("DATABASE_URL").unwrap();
        Ok(Self {
            db: Database::connect(db_url).await?,
        })
    }

    async fn get_track_by_id(&self, id: &str) -> Result<Option<track::Model>, DbErr> {
        Track::find_by_id(id).one(&self.db).await
    }

    async fn get_album_by_id(&self, id: &str) -> Result<Option<album::Model>, DbErr> {
        Album::find_by_id(id).one(&self.db).await
    }

    async fn get_albums_to_update(
        &self,
        album_ids_fetched: &HashSet<String>,
    ) -> Result<HashSet<String>, DbErr> {
        let mut update_needed = album_ids_fetched.clone();
        let already_completed: HashSet<String> = Album::find()
            .filter(album::Column::Updated.eq(get_date(0).date_naive()))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|album| album.id.clone())
            .collect::<HashSet<String>>();
        update_needed.retain(|id| !already_completed.contains(id));
        Ok(update_needed)
    }
    async fn get_artist_by_id(&self, id: &str) -> Result<Option<artist::Model>, DbErr> {
        Artist::find_by_id(id).one(&self.db).await
    }
    pub async fn get_artist_by_id_active(
        &self,
        id: &str,
    ) -> Result<Option<artist::ActiveModel>, DbErr> {
        match Artist::find_by_id(id).one(&self.db).await? {
            None => Ok(None),
            Some(value) => Ok(Some(value.into_active_model())),
        }
    }
    pub async fn get_all_artists<P>(&self, f: fn(Vec<artist::Model>) -> P) -> Result<P, DbErr> {
        let artists = Artist::find().all(&self.db).await?;
        Ok(f(artists))
    }
    pub async fn get_daily_streams_by_track(
        &self,
        track: &track::Model,
        sort: daily_streams::Column,
        limit: u64,
    ) -> Result<Vec<daily_streams::Model>, DbErr> {
        track
            .find_related(DailyStreams)
            .order_by_desc(sort)
            .limit(limit)
            .all(&self.db)
            .await
    }

    pub async fn compare_streams(
        &self,
        id: &str,
        playcount: u64,
    ) -> Result<Option<bool>, Box<dyn Error>> {
        let track = self.get_track_by_id(id).await?;

        if track.is_none() {
            return Ok(None);
        }

        let track = track.unwrap();

        let ds = self
            .get_daily_streams_by_track(&track, daily_streams::Column::Date, 3)
            .await?;
        let count = ds.len();

        if count == 0 {
            return Ok(Some(true));
        }

        if ds[0].streams as u64 != playcount || (count >= 2 && ds[0].streams - ds[1].streams <= 100)
        {
            return Ok(Some(true));
        }
        Ok(Some(false))
    }

    pub async fn initial_status_check(id: &str) -> Result<bool, Box<dyn Error>> {
        let updated_track = TrackUnion::get_union(id).await?;
        while {
            let db = DB::create().await?;
            let value = db.compare_streams(id, updated_track.playcount).await?;
            value.is_some() && !value.unwrap()
            //add end of day check
        } {
            println!(
                "Song: {} current stream count {}",
                updated_track.name, updated_track.playcount
            );
            println!("Not ready for update, waiting 15 min");
            sleep(Duration::from_secs(900)).await;
        }
        Ok(true)
    }

    pub async fn update_artist_detail(&self, artists: &[String]) -> Result<bool, Box<dyn Error>> {
        let response = get_artist_data(format!(
            "{}/{}?ids={}",
            "https://api.spotify.com/v1",
            "artists",
            artists.join("%2C")
        ))
        .await?;

        for artist in response {
            let images = artist
                .images
                .iter()
                .map(|image| serde_json::to_string(image).unwrap())
                .collect::<Vec<String>>();

            let active = artist::ActiveModel {
                name: Set(artist.name.to_owned()),
                images: Set(images),
                id: Set(artist.id.to_owned()),
            };

            Artist::insert(active)
                .on_conflict(
                    OnConflict::column(artist::Column::Id)
                        .update_columns([artist::Column::Images, artist::Column::Name])
                        .to_owned(),
                )
                .exec(&self.db)
                .await?;

            FollowerInstance::insert(follower_instance::ActiveModel {
                artist_id: Set(artist.id.to_owned()),
                count: Set(artist.followers.total as i32),
                date: Set(get_date(1).date_naive()),
            })
            .on_conflict(
                OnConflict::columns([
                    follower_instance::Column::Date,
                    follower_instance::Column::ArtistId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&self.db)
            .await?;
        }
        Ok(true)
    }

    pub async fn create_artist(&self, id: &str) -> Result<bool, Box<dyn Error>> {
        let to_create = vec![id.to_string()];
        self.update_artist_detail(&to_create).await
    }

    pub async fn delete_associated_albums(&self, id: &str) -> Result<u64, DbErr> {
        let mut albums = Album::find()
            .find_with_related(Artist)
            .filter(
                Condition::any().add(
                    album::Column::Id.in_subquery(
                        Query::select()
                            .column(artist_albums::Column::AlbumId)
                            .from(ArtistAlbums)
                            .and_where(artist_albums::Column::ArtistId.eq(id.to_owned()))
                            .to_owned(),
                    ),
                ),
            )
            .all(&self.db)
            .await?;
        println!("{}", albums.len());
        albums.retain(|(a, b)| b.len() == 1);
        let to_delete = albums
            .iter()
            .map(|value| value.0.id.clone())
            .collect::<Vec<String>>();
        let result = Album::delete_many()
            .filter(album::Column::Id.is_in(to_delete))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }

    pub async fn delete_artist(&self, id: &str) -> Result<bool, DbErr> {
        self.delete_associated_albums(id).await?;
        let result = Artist::delete_by_id(id).exec(&self.db).await?;
        if result.rows_affected != 0 {
            return Ok(true);
        };
        Ok(false)
    }
    pub async fn update_artists(&self) -> Result<bool, Box<dyn Error>> {
        let artist_ids = self
            .get_all_artists::<Vec<String>>(|value: Vec<artist::Model>| {
                value.iter().map(|x| x.id.clone()).collect::<Vec<String>>()
            })
            .await?;
        self.update_artist_detail(&artist_ids).await?;
        Ok(true)
    }

    #[async_recursion]
    pub async fn get_album_ids(artist: &HashSet<String>, attempt: u32) -> Option<HashSet<String>> {
        dotenv::dotenv().ok();
        let url = env::var("ARTIST_END_POINT").unwrap();
        if artist.is_empty() || attempt == 13 {
            return None;
        }

        let response_bodies: Vec<Result<Vec<String>, String>> =
            future::join_all(artist.iter().map(|artist_id| {
                println!("artist request: {:?}", artist_id);
                let url = &url;
                async move {
                    get_data::<Vec<String>>(url.as_str(), "artistID", artist_id.as_str()).await
                }
            }))
            .await;

        let mut ids = Vec::new();
        let mut artist_errors = HashSet::new();

        for response in response_bodies {
            match response {
                Ok(value) => ids.push(value),
                Err(error) => {
                    artist_errors.insert(error.clone());
                }
            }
        }

        let flat_ids = ids.into_iter().flatten().collect::<HashSet<String>>();
        match DB::get_album_ids(&artist_errors, attempt + 1).await {
            None => Some(flat_ids),
            Some(value) => {
                let mut to_return = value.clone();
                to_return.extend(flat_ids);
                Some(to_return)
            }
        }
    }

    async fn update_albums_3(albums: HashSet<String>, artists: &HashSet<String>) {
        let chunk = 50;
        let response_bodies = stream::iter(albums)
            .map(|id| async move {
                match AlbumUnion::get_union(id.as_str()).await {
                    Ok(value) => value.update(artists).await,
                    Err(error) => Err(Box::from(format!("Error fetching album {}", error))),
                }
            })
            .buffer_unordered(chunk);

        response_bodies
            .for_each(|resp| async {
                match resp {
                    Ok(value) => {
                        println!("Update success {}", value.last_insert_id);
                    }
                    Err(e) => {
                        println!("Update failed {}", e);
                    }
                }
            })
            .await;
    }

    async fn update_albums_2(
        &self,
        album_ids_fetched: &HashSet<String>,
        artists: &HashSet<String>,
    ) -> Result<(), DbErr> {
        let mut albums;
        let mut attempt = 0;
        while {
            albums = self.get_albums_to_update(album_ids_fetched).await?;
            attempt += 1;
            !albums.is_empty() && attempt <= 13
        } {
            DB::update_albums_3(albums, artists).await
        }
        Ok(())
    }
    pub async fn update_albums_1(&self) -> Result<bool, Box<dyn Error>> {
        let artist_ids = self
            .get_all_artists::<HashSet<String>>(|value: Vec<artist::Model>| {
                value
                    .iter()
                    .map(|x| x.id.clone())
                    .collect::<HashSet<String>>()
            })
            .await?;

        match DB::get_album_ids(&artist_ids, 0).await {
            None => Err(Box::from("Fetching Album IDS failed")),
            Some(value) => {
                self.update_albums_2(&value, &artist_ids).await?;
                Ok(true)
            }
        }
    }

    pub async fn tracks_to_update(&self) -> Result<HashSet<String>, DbErr> {
        Ok(Track::find()
            .filter(
                Condition::any().add(
                    track::Column::Id.not_in_subquery(
                        Query::select()
                            .column(daily_streams::Column::TrackId)
                            .from(DailyStreams)
                            .and_where(daily_streams::Column::Date.eq(get_date(1).date_naive()))
                            .to_owned(),
                    ),
                ),
            )
            .all(&self.db)
            .await?
            .iter()
            .map(|model| model.album_id.clone())
            .collect::<HashSet<String>>())
    }

    async fn update_tracks_by_album(albums: HashSet<String>) {
        let chunk = 50;
        let response_bodies = stream::iter(albums)
            .map(|id| async move {
                match AlbumUnion::get_union(id.as_str()).await {
                    Ok(value) => value.update_track_streams().await,
                    Err(error) => Err(Box::from(format!("Error fetching album {}", error))),
                }
            })
            .buffer_unordered(chunk);

        response_bodies
            .for_each(|resp| async {
                match resp {
                    Ok(value) => {
                        println!("Update success {}", value);
                    }
                    Err(e) => {
                        println!("Update failed {}", e);
                    }
                }
            })
            .await;
    }
    pub async fn update_remaining_tracks() -> Result<bool, Box<dyn Error>> {
        let mut db = DB::create().await?;
        let mut albums = db.tracks_to_update().await?;
        loop {
            db = DB::create().await?;
            DB::update_tracks_by_album(albums).await;
            albums = db.tracks_to_update().await?;
            if albums.is_empty() {
                break;
            }
            println!("Tracks not ready to update, waiting 15 min");
            sleep(Duration::from_secs(900)).await;
        }
        Ok(true)
    }

    pub async fn get_album_for_display(id: &str) -> Result<AlbumDisplay, DbErr> {
        AlbumDisplay::create_album(id).await
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AlbumDisplay {
    name: String,
    date: Option<Date>,
    release_date: Date,
    colors: Option<ExtractedColors>,
    images: Vec<Image>,
    sharing_id: String,
    tracks: Vec<TrackRow>,
    total_streams: i64,
}

impl AlbumDisplay {
    async fn create_album(id: &str) -> Result<Self, DbErr> {
        let db = DB::create().await?;
        let album = db.get_album_by_id(id).await?.unwrap();
        let tracks = album.find_related(Track).all(&db.db).await?;
        let images = album
            .images
            .iter()
            .map(|image| serde_json::from_str::<Image>(image).unwrap())
            .collect::<Vec<Image>>();
        let colors: Option<ExtractedColors> = if album.colors.is_none() {
            None
        } else {
            Some(serde_json::from_value::<ExtractedColors>(album.colors.unwrap()).unwrap())
        };
        let mut total = 0i64;
        let mut track_rows = Vec::new();

        for track in tracks {
            let track_row = TrackRow::create_row(&track).await?;
            total += track_row.total.unwrap_or_else(|| 0);
            track_rows.push(track_row);
        }

        Ok(Self {
            name: album.name.to_owned(),
            date: album.updated,
            release_date: album.release_date.to_owned(),
            colors: colors,
            images: images,
            sharing_id: album.sharing_id.to_owned(),
            tracks: track_rows,
            total_streams: total,
        })
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct TrackRow {
    name: String,
    total: Option<i64>,
    difference_day: Option<i64>,
    difference_week: Option<i64>,
}

impl TrackRow {
    async fn create_row(track: &track::Model) -> Result<Self, DbErr> {
        let db = DB::create().await?;
        let ds = db
            .get_daily_streams_by_track(&track, daily_streams::Column::Date, 8)
            .await?;
        Ok(Self {
            name: track.name.to_owned(),
            total: if ds.len() == 0 {
                None
            } else {
                Some(ds[0].streams)
            },
            difference_day: if ds.len() < 2 {
                None
            } else {
                Some(ds[0].streams - ds[1].streams)
            },
            difference_week: if ds.len() < 8 {
                None
            } else {
                Some(ds[0].streams - ds[7].streams)
            },
        })
    }
}

pub fn get_date(num: u64) -> DateTime<Utc> {
    let date = Local::now().checked_sub_days(Days::new(num)).unwrap();
    Utc.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .unwrap()
}
