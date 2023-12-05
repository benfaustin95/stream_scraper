use crate::album_union::Duration;
use crate::http_requests;
use crate::http_requests::GetUnion;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::env;

/// Used throughout the library to deserialize standard image JSON objects.
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Image {
    url: String,
    height: u32,
    width: u32,
}

/// Used throughout the library to deserialized standard sharing information JSON objects.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharingInfo {
    pub share_url: String,
    pub share_id: String,
}

/// The Track Union struct is used to deserialize and ingest the track information scraped from
/// the spotify web player.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackUnion {
    #[serde(alias = "__typename")]
    typename: String,
    id: String,
    uri: String,
    pub(crate) name: String,
    duration: Duration,
    track_number: u32,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub(crate) playcount: u64,
    sharing_info: SharingInfo,
}

/// The Track Union Implementation of the GetUnion Trait fetches the track from the aws endpoint I created
/// which intercepts the network traffic from spotify's web player in order ot obtain the playcount
/// for track in the album (as well as other track details).
#[async_trait]
impl GetUnion for TrackUnion {
    async fn get_union<'a>(id: &str) -> Result<Self, String> {
        dotenv::dotenv().ok();
        let key = env::var("TRACK_END_POINT").unwrap();
        http_requests::get_union::<Self>(key.as_str(), id, "trackID").await
    }
}

#[tokio::test]
async fn test_get_track_union() {
    let union = TrackUnion::get_union("7Eb9KO7l6Qt8skHG9oRQBD")
        .await
        .ok()
        .unwrap();
    assert_eq!(union.id, "7Eb9KO7l6Qt8skHG9oRQBD");
}
