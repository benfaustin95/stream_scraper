use rocket::serde::json::Json;
use rocket::*;
use sea_orm::DbErr;
use stream_accumulator::entity::artist;
use stream_accumulator::modules::artist_display::{AlbumDisplay, ArtistDisplay};
use stream_accumulator::modules::data_base::DB;

#[derive(Responder)]
#[response(status = 500, content_type = "json")]
struct ErrorResponder {
    message: String,
}

impl From<DbErr> for ErrorResponder {
    fn from(err: DbErr) -> ErrorResponder {
        ErrorResponder {
            message: err.to_string(),
        }
    }
}

impl From<String> for ErrorResponder {
    fn from(string: String) -> ErrorResponder {
        ErrorResponder { message: string }
    }
}

impl From<&str> for ErrorResponder {
    fn from(str: &str) -> ErrorResponder {
        str.to_owned().into()
    }
}

#[post("/artists/create/<id>")]
async fn create_artist(db: &State<DB>, id: &str) -> Result<Json<artist::Model>, ErrorResponder> {
    let db = db as &DB;
    match db.create_artist(id).await {
        Some(value) => Ok(Json(value)),
        _ => Err(ErrorResponder::from("Artist not created")),
    }
}

#[post("/artists/delete/<id>")]
async fn delete_artist(db: &State<DB>, id: &str) -> Result<String, ErrorResponder> {
    let db = db as &DB;
    match db.delete_artist(id).await {
        Ok(true) => Ok(format!("Artist {} deleted", id)),
        _ => Err(ErrorResponder::from("Artist not deleted")),
    }
}
#[get("/artists")]
async fn artists(db: &State<DB>) -> Json<Vec<artist::Model>> {
    let db = db as &DB;
    let artists = db
        .get_all_artists_standard(|artists: Vec<artist::Model>| artists)
        .await
        .unwrap();
    Json(artists)
}

#[get("/artists/display/<id>")]
async fn artist_display(id: &str) -> Result<Json<ArtistDisplay>, ErrorResponder> {
    match DB::get_artist_for_display(id).await {
        Ok(Some(value)) => Ok(Json(value)),
        _ => Err(ErrorResponder::from(format!(
            "Error fetching artist {}",
            id
        ))),
    }
}

#[get("/album/display/<id>")]
async fn album_display(id: &str) -> Result<Json<AlbumDisplay>, ErrorResponder> {
    match DB::get_album_for_display(id).await {
        Ok(value) => Ok(Json(value)),
        _ => Err(ErrorResponder::from(format!("Error fetching album {}", id))),
    }
}

#[launch]
pub async fn rocket() -> Rocket<Build> {
    let db = match DB::create().await {
        Ok(db) => db,
        Err(error) => panic!("error with database: {}", error),
    };
    build().manage(db).mount(
        "/",
        routes![
            artists,
            create_artist,
            delete_artist,
            artist_display,
            album_display
        ],
    )
}
