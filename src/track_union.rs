use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;
use crate::get_data;

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
struct SharingInfo {
    share_url: String,
    share_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackUnion {
    #[serde(alias="__typename")]
    typename: String,
    id: String,
    uri: String,
    name: String,
    content_rating: ContentRating,
    duration: Duration,
    track_number: u32,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    playcount: u64,
    sharing_info: SharingInfo,
}


pub async fn get_union<T: for<'a> Deserialize<'a>>(url: &str, id: &str) -> Result<T, reqwest::Error> {
    let mut body = HashMap::new();
    body.insert("trackID", id);
    get_data::<T>(
        url,
        body,
    ).await
}
