use crate::track_union::Image;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

#[async_trait]
pub(crate) trait GetUnion {
    async fn get_union<'a>(id: &str) -> Result<Self, String>
    where
        Self: Sized;
}

#[derive(Deserialize, Serialize, Debug)]
struct AccessToken {
    access_token: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct FollowersAPI {
    href: Option<String>,
    pub(crate) total: u64,
}
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct ArtistAPI {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) images: Vec<Image>,
    pub(crate) followers: FollowersAPI,
}

#[derive(Deserialize, Serialize, Debug)]
struct ArtistsAPI {
    artists: Vec<ArtistAPI>,
}

async fn get_spotify_access_token() -> Result<AccessToken, reqwest::Error> {
    let client = reqwest::Client::new();
    let key = env::var("SPOTIFY_KEY").unwrap();
    client
        .post("https://accounts.spotify.com/api/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(key.clone())
        .send()
        .await?
        .json::<AccessToken>()
        .await
}

pub(crate) async fn get_artist_data(url: String) -> Result<Vec<ArtistAPI>, reqwest::Error> {
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

pub(crate) async fn get_union<T: for<'a> Deserialize<'a>>(
    url: &str,
    id: &str,
    key: &str,
) -> Result<T, String> {
    get_data::<T>(url, key, id).await
}

pub(crate) async fn get_data<T: for<'a> Deserialize<'a>>(
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
                println!("request failed: {:?}", res);
                Err(value.to_owned())
            }
        },
    }
}
