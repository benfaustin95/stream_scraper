use crate::entity::prelude::{Album, DailyStreams, Track};
use crate::entity::{album, daily_streams, track};
use crate::track_union::SharingInfo;
use crate::{get_date, Image, DB};
use chrono::{DateTime, Local, TimeZone, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, EntityTrait, InsertResult};
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::collections::HashSet;
use std::error::Error;

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
    sources: Vec<Image>,
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
        db: &DB,
        artist_map: &HashSet<&str>,
    ) -> Result<InsertResult<album::ActiveModel>, Box<dyn Error>> {
        let mut images = Vec::new();

        for image in self.cover_art.sources.iter() {
            images.push(serde_json::to_string(image).unwrap())
        }

        let active_album = album::ActiveModel {
            id: Set(self.uri.split(":").collect::<Vec<&str>>()[2].to_owned()),
            name: Set(self.name.to_owned()),
            release_date: Set(DateTime::parse_from_rfc3339(&self.date.iso_string)
                .unwrap()
                .date_naive()),
            album_type: Set(self.album_type.to_owned()),
            images: Set(images),
            colors: Set(Some(serde_json::json!(&self.cover_art.extracted_colors))),
            display: Set(true),
            updated: Set(Some(get_date(0).date_naive())),
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

        //hook up artists to album

        for track in self.tracks.items.iter() {
            match track
                .update(result.last_insert_id.as_str(), artist_map, db)
                .await
            {
                None => continue,
                _ => (),
            }
            track.update_streams(&db).await;
        }
        Ok(result)
    }
}

impl TrackObject {
    pub async fn update(
        &self,
        album_id: &str,
        artist_map: &HashSet<&str>,
        db: &DB,
    ) -> Option<&str> {
        for i in 0..self.track.artists.items.len() {
            let artist_id = self.track.artists.items[i]
                .uri
                .split(":")
                .collect::<Vec<&str>>()[2];
            if artist_map.contains(&artist_id) {
                break;
            } else if i + 1 == self.track.artists.items.len() {
                return None;
            }
        }

        let track_id = self.track.uri.split(":").collect::<Vec<&str>>()[2];
        let active_track = track::ActiveModel {
            id: Set(track_id.to_owned()),
            album_id: Set(album_id.to_owned()),
            name: Set(self.track.name.to_owned()),
            length: Set(self.track.duration.total_milliseconds as i32),
        };

        match Track::insert(active_track)
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
            .await
        {
            Ok(value) => println!("track updated: {}", value.last_insert_id),
            Err(error) => {
                println!("Error updating track: {}", error);
                return None;
            }
        }
        //hook up artists to track
        Some(track_id)
    }

    async fn update_streams(&self, db: &DB) -> Option<bool> {
        let track_id = self.track.uri.split(":").collect::<Vec<&str>>()[2];
        match db.compare_streams(track_id, self.track.playcount).await {
            Ok(value) => {
                if !value {
                    return None;
                }
            }
            Err(error) => {
                println!("Error checking streams for {}: {}", track_id, error);
                return None;
            }
        }

        let active_daily_streams = daily_streams::ActiveModel {
            date: Set(get_date(1).date_naive()),
            track_id: Set(track_id.to_owned()),
            streams: Set(self.track.playcount as i64),
            time: Set(chrono::Utc::now()
                .with_timezone(&Local.offset_from_utc_date(&Utc::now().date_naive()))),
        };

        match DailyStreams::insert(active_daily_streams)
            .on_conflict(
                OnConflict::columns([daily_streams::Column::Date, daily_streams::Column::TrackId])
                    .update_columns([daily_streams::Column::Streams, daily_streams::Column::Time])
                    .to_owned(),
            )
            .exec(&db.db)
            .await
        {
            Ok(value) => println!("Daily streams update: {:?}", value.last_insert_id),
            Err(error) => {
                println!("Error updating streams: {}", error);
                return None;
            }
        }
        Some(true)
    }
}
