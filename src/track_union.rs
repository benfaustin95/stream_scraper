use crate::http_requests;
use crate::http_requests::GetUnion;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::env;

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Image {
    url: String,
    height: u32,
    width: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContentRating {
    label: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Duration {
    total_milliseconds: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharingInfo {
    pub share_url: String,
    pub share_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackUnion {
    #[serde(alias = "__typename")]
    typename: String,
    id: String,
    uri: String,
    pub(crate) name: String,
    content_rating: ContentRating,
    duration: Duration,
    track_number: u32,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub(crate) playcount: u64,
    sharing_info: SharingInfo,
}

#[async_trait]
impl GetUnion for TrackUnion {
    async fn get_union<'a>(id: &str) -> Result<Self, String> {
        dotenv::dotenv().ok();
        let key = env::var("TRACK_END_POINT").unwrap();
        http_requests::get_union::<Self>(key.as_str(), id, "trackID").await
    }
}
