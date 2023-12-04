//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.6

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "daily_streams")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub date: Date,
    #[sea_orm(primary_key, auto_increment = false)]
    pub track_id: String,
    pub time: DateTimeWithTimeZone,
    pub streams: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::track::Entity",
        from = "Column::TrackId",
        to = "super::track::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Track,
}

impl Related<super::track::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Track.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
