use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::collections::HashMap;

#[async_trait]
pub trait GetUnion {
    async fn get_union<'a>(id: &str) -> Result<Self, reqwest::Error>
    where
        Self: Sized;
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
pub struct SharingInfo {
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
    name: String,
    content_rating: ContentRating,
    duration: Duration,
    track_number: u32,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub playcount: u64,
    sharing_info: SharingInfo,
}

#[async_trait]
impl GetUnion for TrackUnion {
    async fn get_union<'a>(id: &str) -> Result<Self, reqwest::Error> {
        get_union::<Self>(
            "https://2p3vesqneoheqyxagoxh5wrtay0nednp.lambda-url.us-west-2.on.aws/",
            id,
            "trackID",
        )
        .await
    }
}

pub async fn get_union<T: for<'a> Deserialize<'a>>(
    url: &str,
    id: &str,
    key: &str,
) -> Result<T, reqwest::Error> {
    let mut body = HashMap::new();
    body.insert(key, id);
    get_data::<T>(url, body).await
}

pub async fn get_data<T: for<'a> Deserialize<'a>>(
    url: &str,
    body: HashMap<&str, &str>,
) -> Result<T, reqwest::Error> {
    let client = reqwest::Client::new();
    client.get(url).json(&body).send().await?.json::<T>().await
}
