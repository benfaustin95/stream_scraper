use crate::album_union::ExtractedColors;
use crate::data_base::DB;
use crate::entity::album::Entity as Album;
use crate::entity::track::Entity as Track;
use crate::entity::{album, daily_streams, track};
use crate::track_union::Image;
use chrono::NaiveDate as Date;
use futures::future;
use sea_orm::{DbErr, ModelTrait};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct TrackRow {
    name: String,
    total: Option<i64>,
    difference_day: Option<i64>,
    difference_week: Option<i64>,
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
    total: i64,
    difference_day: i64,
    difference_week: i64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ArtistDisplay {
    name: String,
    images: Vec<Image>,
    albums: Vec<AlbumDisplay>,
}

impl ArtistDisplay {
    pub(crate) async fn create_artist(id: &str) -> Result<Self, DbErr> {
        let db = DB::create().await?;
        let artist = db.get_artist_by_id(id).await?.unwrap();
        let albums = artist
            .find_related(Album)
            .find_with_related(Track)
            .all(&db.db)
            .await?;
        let images = artist
            .images
            .iter()
            .map(|image| serde_json::from_str::<Image>(image).unwrap())
            .collect::<Vec<Image>>();
        let mut albums_out = Vec::new();
        for (i, album) in albums.iter().enumerate() {
            println!("album {} of {}", i, albums.len());
            match AlbumDisplay::create_album(&db, album).await {
                Ok(value) => albums_out.push(value),
                Err(error) => println!("Error creating display album: {}", error),
            }
        }

        Ok(Self {
            name: artist.name,
            images,
            albums: albums_out,
        })
    }
}

impl AlbumDisplay {
    pub(crate) async fn create_album(
        db: &DB,
        album: &(album::Model, Vec<track::Model>),
    ) -> Result<Self, DbErr> {
        let (album, tracks) = album;
        let images = album
            .images
            .iter()
            .map(|image| serde_json::from_str::<Image>(image).unwrap())
            .collect::<Vec<Image>>();
        let colors: Option<ExtractedColors> = if album.colors.is_none() {
            None
        } else {
            Some(
                serde_json::from_value::<ExtractedColors>(album.to_owned().colors.unwrap())
                    .unwrap(),
            )
        };
        let mut track_rows = Vec::new();

        let response_bodies: Vec<Result<TrackRow, DbErr>> = future::join_all(
            tracks
                .iter()
                .map(|track| async move { TrackRow::create_row(db, track).await }),
        )
        .await;

        let mut total = 0;
        let mut difference_day = 0;
        let mut difference_week = 0;

        for response in response_bodies {
            match response {
                Ok(value) => {
                    total += if value.total.is_none() {
                        0
                    } else {
                        value.total.to_owned().unwrap()
                    };
                    difference_day += if value.difference_day.is_none() {
                        0
                    } else {
                        value.difference_day.to_owned().unwrap()
                    };
                    difference_week += if value.difference_week.is_none() {
                        0
                    } else {
                        value.difference_week.to_owned().unwrap()
                    };
                    track_rows.push(value)
                }
                Err(error) => println!("error creating track_row: {}", error),
            }
        }

        Ok(Self {
            name: album.name.to_owned(),
            date: album.updated,
            release_date: album.release_date.to_owned(),
            colors,
            images,
            sharing_id: album.sharing_id.to_owned(),
            tracks: track_rows,
            total,
            difference_day,
            difference_week,
        })
    }
}

impl TrackRow {
    async fn create_row(db: &DB, track: &track::Model) -> Result<Self, DbErr> {
        let ds = db
            .get_daily_streams_by_track(track, daily_streams::Column::Date, 8)
            .await?;
        Ok(Self {
            name: track.name.to_owned(),
            total: if ds.is_empty() {
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
