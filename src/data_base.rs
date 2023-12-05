use crate::album_union::AlbumUnion;
use crate::artist_display::{AlbumDisplay, ArtistDisplay};
use crate::entity::{prelude::*, *};
use crate::http_requests::{get_artist_albums, get_artist_detail, get_data, GetUnion};
use crate::track_union::TrackUnion;
use async_recursion::async_recursion;
use chrono::{DateTime, Datelike, Days, Local, TimeZone, Utc};
use futures::{future, stream, StreamExt};
use sea_orm::{
    sea_query::{OnConflict, Query},
    ColumnTrait, Condition, Database, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    ModelTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::{collections::HashSet, env, error::Error};
use tokio::time::{sleep, Duration};

/// DB struct houses primary client interface used to direct application
pub struct DB {
    pub db: DatabaseConnection,
}

impl DB {
    /// Creates and returns a DB struct with an active database connection.
    pub async fn create() -> Result<Self, DbErr> {
        dotenv::dotenv().ok();
        let db_url = env::var("DATABASE_URL").unwrap();
        Ok(Self {
            db: Database::connect(db_url).await?,
        })
    }

    /// Fetches and returns the track model associated with the given id from the database.
    async fn get_track_by_id(&self, id: &str) -> Result<Option<track::Model>, DbErr> {
        Track::find_by_id(id).one(&self.db).await
    }

    /// Fetches and returns the album model associated with the given id from the database.
    pub async fn get_album_by_id(&self, id: &str) -> Result<Option<album::Model>, DbErr> {
        Album::find_by_id(id).one(&self.db).await
    }

    /// Fetches returns the augmented HashSet of album ids such that the set only contains IDS
    /// that have not been updated for the current date.
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

    /// Fetches and returns the artist model for a given id from the database.
    pub(crate) async fn get_artist_by_id(&self, id: &str) -> Result<Option<artist::Model>, DbErr> {
        Artist::find_by_id(id).one(&self.db).await
    }

    /// Fetches and returns the active artist model for a given id from the database.
    pub async fn get_artist_by_id_active(
        &self,
        id: &str,
    ) -> Result<Option<artist::ActiveModel>, DbErr> {
        match Artist::find_by_id(id).one(&self.db).await? {
            None => Ok(None),
            Some(value) => Ok(Some(value.into_active_model())),
        }
    }

    /// Fetches and returns all artist models from the database.
    async fn get_all_artists_standard<P>(
        &self,
        f: fn(Vec<artist::Model>) -> P,
    ) -> Result<P, DbErr> {
        let artists = Artist::find().all(&self.db).await?;
        Ok(f(artists))
    }
    pub(crate) async fn get_daily_streams_by_track(
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

    /// Compares the most recently tracked streaming count with what was intercepted from spotify's web player.
    /// If the track is ready to be updated true or none is returned, else false is returned.
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

    /// Determines whether the given track id is ready to be updated, if true is returned the daily
    /// update process will begin.
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

    /// Updates the artist detail for all artists within the given slice of artist Ids.
    pub async fn update_artist_detail(&self, artists: &[String]) -> Result<bool, Box<dyn Error>> {
        let response = get_artist_detail(format!(
            "{}/{}?ids={}",
            "https://api.spotify.com/v1",
            "artists",
            artists.join("%2C")
        ))
        .await?;

        if response.is_empty() {
            return Ok(false);
        }

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

    /// Creates the artists associated with the given id, once created they will be tracked until deleted.
    pub async fn create_artist(&self, id: &str) -> Result<bool, Box<dyn Error>> {
        if id == "5K4W6rqBFWDnAN6FQUkS6x" {
            return Ok(false);
        }
        let to_create = vec![id.to_string()];
        self.update_artist_detail(&to_create).await
    }

    /// Deletes the albums associated with only the artist ID supplied.
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
        albums.retain(|(_, b)| b.len() == 1);
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

    /// Deletes the artist and all associated information in the database.
    pub async fn delete_artist(&self, id: &str) -> Result<bool, DbErr> {
        if id == "06HL4z0CvFAxyc27GXpf02" {
            return Ok(false);
        }
        self.delete_associated_albums(id).await?;
        let result = Artist::delete_by_id(id).exec(&self.db).await?;
        if result.rows_affected != 0 {
            return Ok(true);
        };
        Ok(false)
    }

    /// Update artists fetches all artist ids from the data base then calls update artist detail.
    pub async fn update_artists(&self) -> Result<bool, Box<dyn Error>> {
        let artist_ids = self
            .get_all_artists_standard::<Vec<String>>(|value: Vec<artist::Model>| {
                value.iter().map(|x| x.id.clone()).collect::<Vec<String>>()
            })
            .await?;
        self.update_artist_detail(&artist_ids).await
    }

    /// Fetches all artist IDs from two points, all single, compilation, and album ids are fetched
    /// directly from the spotify web API, while appears_on albums are scraped from the web player.
    #[async_recursion]
    async fn get_album_ids(artist: &HashSet<String>, attempt: u32) -> Option<HashSet<String>> {
        if artist.is_empty() || attempt == 13 {
            return None;
        }

        let mut response_bodies: Vec<Result<Vec<String>, String>> =
            future::join_all(artist.iter().map(|artist_id| {
                println!("artist request: {:?}", artist_id);
                async move { get_artist_albums(artist_id).await }
            }))
            .await;
        response_bodies.extend(
            future::join_all(artist.iter().map(|artist_id| {
                println!("artist appears on request: {:?}", artist_id);
                async move {
                    get_data::<Vec<String>>(
                        env::var("ARTIST_END_POINT").unwrap().as_str(),
                        "artistID",
                        artist_id,
                    )
                    .await
                }
            }))
            .await,
        );

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

    /// Update albums 3 handles the final stage of the album update process getting the scraped album
    /// union from the web player and using it to update/create the album in the database.
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

    /// Update albums 2 handles iterating through available album ids until all have been updated.
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

    /// Update albums 1 fetches all album ids associated with tracked artists and calls stage 2
    pub async fn update_albums_1(&self) -> Result<bool, Box<dyn Error>> {
        let artist_ids = self
            .get_all_artists_standard::<HashSet<String>>(|value: Vec<artist::Model>| {
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

    /// Tracks to update return the album ids of all tracks whose streams have not been updated for the current date.
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

    /// Update track by album handles the final stage of the dail update process getting the scraped album
    /// union from the web player and using it to update/create the album in the database.
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

    /// Update remaining tracks iterates until no tracks remain that have not been updated.
    pub async fn update_remaining_tracks() -> Result<bool, Box<dyn Error>> {
        let mut db = DB::create().await?;
        let mut albums = db.tracks_to_update().await?;
        let mut attempt = 0;
        loop {
            db = DB::create().await?;
            DB::update_tracks_by_album(albums).await;
            albums = db.tracks_to_update().await?;
            if albums.is_empty() && attempt < 13 {
                break;
            }
            attempt += 1;
            println!("Tracks not ready to update, waiting 15 min");
            sleep(Duration::from_secs(900)).await;
        }
        Ok(true)
    }

    /// Get album for display returns an Album display object containing most recent streaming
    /// information of each track.
    pub async fn get_album_for_display(id: &str) -> Result<AlbumDisplay, DbErr> {
        let db = DB::create().await?;
        let album = Album::find_by_id(id)
            .find_with_related(Track)
            .all(&db.db)
            .await?;
        AlbumDisplay::create_album(&db, &album[0]).await
    }
    pub async fn get_artist_for_display(id: &str) -> Result<ArtistDisplay, DbErr> {
        ArtistDisplay::create_artist(id).await
    }

    /// Daily update guides the flow of the (current) primary component of the application, updating
    /// the database with the current daily information.
    pub async fn daily_update() -> Result<chrono::Duration, Box<dyn Error>> {
        let db = DB::create().await?;
        DB::initial_status_check(env::var("STATUS_CHECK_SONG_ID")?.as_str())
            .await
            .map_err(|error| {
                println!("Error: {}", error);
                error
            })?;

        println!("Passed status check");
        let now = Local::now();

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
        DB::update_remaining_tracks().await.map_err(|error| {
            println!("Error updating remaining tracks: {}", error);
            error
        })?;

        Ok(Local::now() - now)
    }
}

/// Get date returns the date (MM DD YYYY 00:00) of the given day offset.
pub fn get_date(offset: u64) -> DateTime<Utc> {
    let date = Local::now().checked_sub_days(Days::new(offset)).unwrap();
    Utc.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use crate::data_base::DB;
    use crate::entity::{prelude::*, *};
    use sea_orm::EntityTrait;

    #[tokio::test]
    async fn test_create_db() {
        assert!(DB::create().await.ok().is_some())
    }

    #[tokio::test]
    async fn test_all_artists() {
        let db_option = DB::create().await.ok();
        assert!(db_option.is_some());
        let db = db_option.unwrap();
        let test_fn = |artists: Vec<artist::Model>| {
            artists
                .into_iter()
                .map(|model| model.id.to_owned())
                .collect::<Vec<String>>()
        };
        assert!(db
            .get_all_artists_standard::<Vec<String>>(test_fn)
            .await
            .ok()
            .is_some());
    }
    #[tokio::test]
    async fn test_get_artist_by_id() {
        let db_option = DB::create().await.ok();
        assert!(db_option.is_some());
        let db = db_option.unwrap();

        assert!(db
            .get_artist_by_id("06HL4z0CvFAxyc27GXpf02")
            .await
            .ok()
            .unwrap()
            .is_some());
        assert!(db
            .get_artist_by_id("4q3ewBCX7sLwd24euuV69X")
            .await
            .ok()
            .unwrap()
            .is_some());
        assert!(db
            .get_artist_by_id("66CXWjxzNUsdJxJ2JdwvnR")
            .await
            .ok()
            .unwrap()
            .is_some());
        assert!(db
            .get_artist_by_id("5K4W6rqBFWDnAN6FQUkS6x")
            .await
            .ok()
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_get_track_by_id() {
        let db_option = DB::create().await.ok();
        assert!(db_option.is_some());
        let db = db_option.unwrap();
        let artist = Artist::find_by_id("06HL4z0CvFAxyc27GXpf02")
            .find_with_related(Track)
            .all(&db.db)
            .await
            .ok();
        assert!(artist.is_some());

        for (i, item) in artist.unwrap()[0].1.iter().enumerate() {
            assert!(db
                .get_track_by_id(item.id.as_str())
                .await
                .ok()
                .unwrap()
                .is_some());

            if i > 10 {
                break;
            }
        }

        assert!(db
            .get_track_by_id("6bKPmj3k2zoTzoE")
            .await
            .ok()
            .unwrap()
            .is_none());
        assert!(db
            .get_track_by_id("55roS1go7otdIVY")
            .await
            .ok()
            .unwrap()
            .is_none());
        assert!(db
            .get_track_by_id("3ZKRAzNAsiJrBGU")
            .await
            .ok()
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_get_album_by_id() {
        let db_option = DB::create().await.ok();
        assert!(db_option.is_some());
        let db = db_option.unwrap();
        let artist = Artist::find_by_id("06HL4z0CvFAxyc27GXpf02")
            .find_with_related(Album)
            .all(&db.db)
            .await
            .ok();
        assert!(artist.is_some());

        for (i, item) in artist.unwrap()[0].1.iter().enumerate() {
            assert!(db
                .get_album_by_id(item.id.as_str())
                .await
                .ok()
                .unwrap()
                .is_some());
            if i > 10 {
                break;
            }
        }
        assert!(db
            .get_album_by_id("6bKPmj3k2zoTzoE")
            .await
            .ok()
            .unwrap()
            .is_none());
        assert!(db
            .get_album_by_id("55roS1go7otdIVY")
            .await
            .ok()
            .unwrap()
            .is_none());
        assert!(db
            .get_album_by_id("3ZKRAzNAsiJrBGU")
            .await
            .ok()
            .unwrap()
            .is_none());
    }
}
