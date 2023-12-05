use crate::album_union::get_id_from_uri;
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

#[derive(Deserialize, Serialize, Debug)]
struct SimpleAlbum {
    uri: String,
}
#[derive(Deserialize, Serialize, Debug)]
struct ArtistAlbumFetch {
    next: Option<String>,
    items: Vec<SimpleAlbum>,
    total: u64,
}

async fn get_spotify_access_token() -> Result<AccessToken, reqwest::Error> {
    dotenv::dotenv().ok();
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

pub(crate) async fn get_artist_detail(url: String) -> Result<Vec<ArtistAPI>, reqwest::Error> {
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

pub async fn get_artist_albums(id: &str) -> Result<Vec<String>, String> {
    let types = vec!["album", "single", "compilation"];
    let access_token_object = get_spotify_access_token().await.ok();

    if access_token_object.is_none() {
        return Err(id.to_string());
    }

    let access_token = access_token_object.unwrap();
    let mut to_return = Vec::new();
    let mut next: Option<String>;

    for fetch_type in types {
        match request(format!("https://api.spotify.com/v1/artists/{}/albums?include_groups={}&offset=0&limit=50&locale=en-US,en;q=0.9", id, fetch_type).as_str(), &access_token).await {
            Ok((ids, next_url)) => {
                to_return.extend(ids);
                next = next_url;
            },
            Err(_) => return Err(id.to_string()),
        }
        while next.is_some() {
            match request(next.unwrap().as_str(), &access_token).await {
                Ok((ids, next_url)) => {
                    to_return.extend(ids);
                    next = next_url;
                }
                Err(_) => return Err(id.to_string()),
            }
        }
    }
    Ok(to_return)
}

async fn request(
    url: &str,
    access_token: &AccessToken,
) -> Result<(Vec<String>, Option<String>), ()> {
    match reqwest::Client::new()
        .get(url)
        .header(
            "Authorization",
            format!("Bearer {}", access_token.access_token),
        )
        .send()
        .await
    {
        Err(_) => Err(()),
        Ok(res) => match res.status() {
            reqwest::StatusCode::OK => match res.json::<ArtistAlbumFetch>().await {
                Ok(res) => Ok((
                    res.items
                        .iter()
                        .map(|album| get_id_from_uri(album.uri.as_str()).to_string())
                        .collect::<Vec<String>>(),
                    res.next,
                )),
                Err(_) => Err(()),
            },
            _ => Err(()),
        },
    }
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
