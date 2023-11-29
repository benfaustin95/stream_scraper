use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::collections::HashMap;

#[async_trait]
pub trait GetUnion {
    async fn get_union<'a>(id: &str) -> Result<Self, String>
    where
        Self: Sized;
}

#[derive(Deserialize, Serialize, Debug)]
struct AccessToken {
    access_token: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FollowersAPI {
    href: Option<String>,
    pub total: u64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Image {
    url: String,
    height: u32,
    width: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ArtistAPI {
    pub id: String,
    pub name: String,
    pub images: Vec<Image>,
    pub followers: FollowersAPI,
}

#[derive(Deserialize, Serialize, Debug)]
struct ArtistsAPI {
    artists: Vec<ArtistAPI>,
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
    async fn get_union<'a>(id: &str) -> Result<Self, String> {
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
) -> Result<T, String> {
    get_data::<T>(url, key, id).await
}

pub async fn get_data<T: for<'a> Deserialize<'a>>(
    url: &str,
    key: &str,
    value: &str,
) -> Result<T, String> {
    let client = reqwest::Client::new();
    let mut body = HashMap::new();
    body.insert(key, value);
    match client.get(url).json(&body).send().await {
        Err(error) => {
            println!("request failed: {}", error);
            Err(value.to_owned())
        }
        Ok(res) => match res.status() {
            reqwest::StatusCode::OK => res
                .json::<T>()
                .await
                .map_or_else(|_| Err(value.to_owned()), |value| Ok(value)),
            _ => {
                println!("issue: {:?}", res);
                Err(value.to_owned())
            }
        },
    }
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

pub async fn get_artist_data(url: String) -> Result<Vec<ArtistAPI>, reqwest::Error> {
    let access_token = get_spotify_access_token().await?;
    Ok(reqwest::Client::new()
        .get(url)
        .header(
            "Authorization",
            format!("Bearer {}", access_token.access_token),
        )
        .send()
        .await?
        .json::<ArtistsAPI>()
        .await?
        .artists)
}
