//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.6

use async_trait::async_trait;
use sea_orm::entity::prelude::*;
use sea_orm::{QueryOrder, QuerySelect};
use crate::entity::daily_streams;
use crate::entity::track::Relation::DailyStreams;
use crate::getDB;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "track")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub album_id: String,
    pub length: i32,
}

#[async_trait]
impl Model {
    async fn compare_streams(&self, playcount: u64) -> Result<bool, DbErr>{
        let db = getDB().await?;
        let ds: Vec<daily_streams::Model> = self.find_related(DailyStreams)
            .order_by_desc(daily_streams::Column::Date)
            .limit(3)
            .all(&db)
            .await?;
        if ds.is_empty() || ds[0].streams != playcount || (ds.len() >=2 && ds[0].streams - ds[1].streams <= 100) {
            return Ok(true);
        }
        println!("{:?} != {}, {}", ds, playcount, self.name);
        return Ok(false)
    }
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::album::Entity",
        from = "Column::AlbumId",
        to = "super::album::Column::Id",
        on_update = "Cascade",
        on_delete = "NoAction"
    )]
    Album,
    #[sea_orm(has_many = "super::artist_tracks::Entity")]
    ArtistTracks,
    #[sea_orm(has_many = "super::daily_streams::Entity")]
    DailyStreams,
}

impl Related<super::album::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Album.def()
    }
}

impl Related<super::artist_tracks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ArtistTracks.def()
    }
}

impl Related<super::daily_streams::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DailyStreams.def()
    }
}

impl Related<super::artist::Entity> for Entity {
    fn to() -> RelationDef {
        super::artist_tracks::Relation::Artist.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::artist_tracks::Relation::Track.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
