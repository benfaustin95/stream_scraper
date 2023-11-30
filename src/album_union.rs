use crate::data_base::DB;
use crate::entity::prelude::{Album, ArtistAlbums, ArtistTracks, DailyStreams, Track};
use crate::entity::{album, artist_albums, artist_tracks, daily_streams, track};
use crate::track_union::{get_union, GetUnion, SharingInfo};
use crate::{data_base, track_union};
use async_trait::async_trait;
use chrono::{DateTime, Local, TimeZone, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::ActiveValue::Set;
use sea_orm::{DbErr, EntityTrait, InsertResult};
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::{collections::HashSet, env, error::Error};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ArtistObject {
    uri: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ArtistsObject {
    items: Vec<ArtistObject>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Color {
    hex: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExtractedColors {
    color_raw: Color,
    color_light: Color,
    color_dark: Color,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CoverArt {
    extracted_colors: ExtractedColors,
    sources: Vec<track_union::Image>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DateObject {
    iso_string: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Duration {
    total_milliseconds: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TrackDetail {
    saved: bool,
    uri: String,
    name: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    playcount: u64,
    duration: Duration,
    artists: ArtistsObject,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TrackObject {
    uid: String,
    track: TrackDetail,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TracksObject {
    items: Vec<TrackObject>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AlbumUnion {
    #[serde(alias = "__typename")]
    typename: String,
    pub uri: String,
    name: String,
    date: DateObject,
    #[serde(alias = "type")]
    album_type: String,
    artists: ArtistsObject,
    cover_art: CoverArt,
    sharing_info: SharingInfo,
    tracks: TracksObject,
}

impl AlbumUnion {
    pub async fn update(
        &self,
        artist_map: &HashSet<String>,
    ) -> Result<InsertResult<album::ActiveModel>, Box<dyn Error>> {
        let db = DB::create().await?;
        let images = self
            .cover_art
            .sources
            .iter()
            .map(|image| serde_json::to_string(image).unwrap())
            .collect::<Vec<String>>();
        let mut connections: Vec<artist_albums::ActiveModel> = Vec::new();
        let album_id = get_id_from_uri(&self.uri);

        for artist in self.artists.items.iter() {
            let artist_id = get_id_from_uri(&artist.uri);
            if artist_map.contains(&artist_id.to_string()) {
                connections.push(artist_albums::ActiveModel {
                    album_id: Set(album_id.to_owned()),
                    artist_id: Set(artist_id.to_owned()),
                });
            }
        }

        let active_album = album::ActiveModel {
            id: Set(album_id.to_owned()),
            name: Set(self.name.to_owned()),
            release_date: Set(DateTime::parse_from_rfc3339(&self.date.iso_string)
                .unwrap()
                .date_naive()),
            album_type: Set(self.album_type.to_owned()),
            images: Set(images),
            colors: Set(Some(serde_json::json!(&self.cover_art.extracted_colors))),
            display: Set(true),
            updated: Set(Some(data_base::get_date(0).date_naive())),
            sharing_id: Set(self.sharing_info.share_id.to_owned()),
        };

        let result = Album::insert(active_album)
            .on_conflict(
                OnConflict::column(album::Column::Id)
                    .update_columns([
                        album::Column::Id,
                        album::Column::Name,
                        album::Column::ReleaseDate,
                        album::Column::AlbumType,
                        album::Column::Images,
                        album::Column::Colors,
                        album::Column::Display,
                        album::Column::Updated,
                        album::Column::SharingId,
                    ])
                    .to_owned(),
            )
            .exec(&db.db)
            .await?;

        ArtistAlbums::insert_many(connections)
            .on_conflict(
                OnConflict::columns([
                    artist_albums::Column::ArtistId,
                    artist_albums::Column::AlbumId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&db.db)
            .await?;

        for track in self.tracks.items.iter() {
            if let Err(error) = track
                .update(result.last_insert_id.as_str(), artist_map, &db)
                .await
            {
                println!("Error updating track {}: {}", track.track.name, error);
                continue;
            }

            if let Err(error) = track.update_streams(&db).await {
                println!(
                    "Error updating track streams {}: {}",
                    track.track.name, error
                );
            }
        }

        Ok(result)
    }

    pub async fn update_track_streams(&self) -> Result<bool, Box<dyn Error>> {
        let db = DB::create().await?;
        let mut updated = 0;
        for track in self.tracks.items.iter() {
            if let Err(error) = track.update_streams(&db).await {
                println!(
                    "Error updating track streams {}: {}",
                    track.track.name, error
                );
            } else {
                updated += 1;
            }
        }
        Ok(updated == self.tracks.items.len())
    }
}

#[async_trait]
impl GetUnion for AlbumUnion {
    async fn get_union<'a>(id: &str) -> Result<Self, String> {
        dotenv::dotenv().ok();
        let key = env::var("ALBUM_END_POINT").unwrap();
        get_union::<Self>(key.as_str(), id, "albumID").await
    }
}

impl TrackObject {
    pub async fn update(
        &self,
        album_id: &str,
        artist_map: &HashSet<String>,
        db: &DB,
    ) -> Result<Option<InsertResult<track::ActiveModel>>, DbErr> {
        let track_id = get_id_from_uri(&self.track.uri);
        let mut connections: Vec<artist_tracks::ActiveModel> = Vec::new();
        for i in 0..self.track.artists.items.len() {
            let artist_id = get_id_from_uri(&self.track.artists.items[i].uri);

            if artist_map.contains(&artist_id.to_string()) {
                connections.push(artist_tracks::ActiveModel {
                    artist_id: Set(artist_id.to_owned()),
                    track_id: Set(track_id.to_owned()),
                });
            }
        }

        if connections.is_empty() {
            return Ok(None);
        }

        let active_track = track::ActiveModel {
            id: Set(track_id.to_owned()),
            album_id: Set(album_id.to_owned()),
            name: Set(self.track.name.to_owned()),
            length: Set(self.track.duration.total_milliseconds as i32),
        };

        let result = Track::insert(active_track)
            .on_conflict(
                OnConflict::column(track::Column::Id)
                    .update_columns([
                        track::Column::Id,
                        track::Column::Name,
                        track::Column::Length,
                        track::Column::AlbumId,
                    ])
                    .to_owned(),
            )
            .exec(&db.db)
            .await?;

        ArtistTracks::insert_many(connections)
            .on_conflict(
                OnConflict::columns([
                    artist_tracks::Column::ArtistId,
                    artist_tracks::Column::TrackId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&db.db)
            .await?;

        Ok(Some(result))
    }

    async fn update_streams(
        &self,
        db: &DB,
    ) -> Result<Option<InsertResult<daily_streams::ActiveModel>>, Box<dyn Error>> {
        let track_id = get_id_from_uri(&self.track.uri);

        match db
            .compare_streams(track_id, self.track.playcount)
            .await
            .unwrap_or_else(|error| {
                println!("Error updating streams: {}", error);
                Some(false)
            }) {
            Some(true) => (),
            _ => return Ok(None),
        }

        let active_daily_streams = daily_streams::ActiveModel {
            date: Set(data_base::get_date(1).date_naive()),
            track_id: Set(track_id.to_owned()),
            streams: Set(self.track.playcount as i64),
            time: Set(chrono::Utc::now()
                .with_timezone(&Local.offset_from_utc_date(&Utc::now().date_naive()))),
        };

        let result = DailyStreams::insert(active_daily_streams)
            .on_conflict(
                OnConflict::columns([daily_streams::Column::Date, daily_streams::Column::TrackId])
                    .update_columns([daily_streams::Column::Streams, daily_streams::Column::Time])
                    .to_owned(),
            )
            .exec(&db.db)
            .await?;
        println!(
            "Updated streams {}: {}",
            self.track.name, self.track.playcount
        );
        Ok(Some(result))
    }
}

fn get_id_from_uri(uri: &str) -> &str {
    uri.split(':').collect::<Vec<&str>>()[2]
}
