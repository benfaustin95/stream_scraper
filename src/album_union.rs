use crate::track_union::SharingInfo;
use crate::Image;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;

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
struct Track {
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
    track: Track,
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
    // artists: ArtistsObject,
    cover_art: CoverArt,
    sharing_info: SharingInfo,
    tracks: TracksObject,
}
