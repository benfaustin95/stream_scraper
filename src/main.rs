use std::env;
use futures::executor::block_on;
use sea_orm::{Database, DbErr};


async fn run() -> Result<(), DbErr> {
    let db_url = env::var("DATABASE_URL").unwrap();
    let db = Database::connect(db_url).await?;
    Ok(())
}
fn main() {
    if let Err(err) = block_on(run()) {
        panic!("{}", err);
    }
}
