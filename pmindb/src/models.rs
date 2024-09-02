//use std::net::Ipv6Addr;

use chrono::NaiveDateTime;
use diesel::deserialize::FromSql;
use diesel::serialize::ToSql;
//  use serde::{Deserialize, Serialize};
use diesel::{deserialize::FromSqlRow, expression::AsExpression, prelude::*, Queryable};

use diesel::{
    backend::Backend,
    deserialize, serialize,
    sql_types::{is_nullable::NotNull, Binary, SqlType},
    sqlite::Sqlite,
};

use crate::db::CreateOrModify;

use pmind_broker::Eui as NodeEui;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Default, Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Binary)]
pub struct Eui(
    // #[diesel(column_name = "eui")]
    NodeEui,
);

impl SqlType for Eui {
    type IsNull = NotNull;
}

impl FromSql<Binary, Sqlite> for Eui {
    fn from_sql(bytes: <Sqlite as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let bytes = <Vec<u8> as FromSql<Binary, Sqlite>>::from_sql(bytes)?;
        if bytes.len() == 6 {
            let mut eui: NodeEui = [0u8; 6];
            eui.copy_from_slice(&bytes);
            Ok(Eui(eui))
        } else {
            Err("incorrect size for Eui type".into())
        }
    }
}

impl ToSql<Binary, Sqlite> for Eui {
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.0.to_vec());
        Ok(serialize::IsNull::No)
    }
}

/// [`PlantRecord`] will record ideal ranges
/// for soil moister, light, and other
/// parameters which will inform the displayed
/// value of when watering is needed
#[derive(Default, Queryable, QueryableByName, PartialEq, Debug, Selectable)]
#[diesel(table_name = crate::schema::plants)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(primary_key(Eui))]
pub(crate) struct PlantRecord {
    /// Each sensor will have a unique EUI, typedef'd as
    /// [`pmind_broker::Eui`] that
    /// persists across resets. This param is converted
    /// to a string and used to
    /// retain associations between sensors and
    /// the plants they are tracking
    eui: Eui,
    /// Each sensor reports it's own plant name
    /// distinct from species. Species is also
    /// self-reported by each sensor and used
    /// to generate the [`SpeciesRecord`]
    name: String,
    species: String,

    pub(crate) addr: String,
    pub(crate) update_count: i32,
    //created_at: NaiveDateTime,
    //updated_at: NaiveDateTime,
    // Desired min & max moisture,
    // based on plant species & growth stage
    // moisture_range: pmindp_sensor::Range<u16>,
    // Desired min & max lux,
    // based on plant species & growth stage
    //  lux_range: pmindp_sensor::Range<f32>,
    // Growth stage
    //  growth_stage: pmindp_sensor::GrowthStage,
}

impl PlantRecord {
    pub fn eui(&self) -> Eui {
        self.eui
    }

    #[allow(unused)]
    pub fn plant_name(&self) -> String {
        self.name.clone()
    }

    #[allow(unused)]
    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn update_count(&self) -> i32 {
        self.update_count
    }
}

#[derive(Insertable, Clone)]
#[diesel(table_name = crate::schema::plants)]
pub(crate) struct NewPlant {
    pub(crate) eui: Eui,
    pub(crate) name: String,
    pub(crate) species: String,
    pub(crate) addr: String,
    pub(crate) update_count: i32,
}

impl NewPlant {
    pub fn new(msg: CreateOrModify) -> Self {
        Self {
            eui: Eui(msg.eui),
            name: msg.name,
            addr: msg.addr.to_string(),
            species: "".to_string(),
            update_count: 0,
        }
    }
}

#[derive(Queryable, PartialEq, Debug, Selectable, Insertable)]
#[diesel(table_name = crate::schema::light_data)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct LightData {
    pub(crate) id: i32,
    pub(crate) parent_plant_eui: Eui,
    pub(crate) lux: f32,
    pub(crate) fs: f32,
    pub(crate) ts: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::light_data)]
pub(crate) struct NewLightData {
    pub(crate) parent_plant_eui: Eui,
    pub(crate) lux: f32,
    pub(crate) fs: f32,
    pub(crate) ts: NaiveDateTime,
}

#[derive(Queryable, PartialEq, Debug, Selectable, Insertable)]
#[diesel(table_name = crate::schema::gas_data)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct GasData {
    pub(crate) id: i32,
    pub(crate) parent_plant_eui: Eui,
    pub(crate) temp: f32,
    pub(crate) pressure: f32,
    pub(crate) humidity: f32,
    pub(crate) gas: f32,
    pub(crate) ts: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::gas_data)]
pub(crate) struct NewGasData {
    pub(crate) parent_plant_eui: Eui,
    pub(crate) temp: f32,
    pub(crate) pressure: f32,
    pub(crate) humidity: f32,
    pub(crate) gas: f32,
    pub(crate) ts: NaiveDateTime,
}

#[derive(Queryable, PartialEq, Debug, Selectable, Insertable)]
#[diesel(table_name = crate::schema::moisture_data)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct MoistureData {
    pub(crate) id: i32,
    pub(crate) parent_plant_eui: Eui,
    pub(crate) moisture: f32,
    pub(crate) temp: f32,
    pub(crate) ts: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::moisture_data)]
pub(crate) struct NewMoistureData {
    pub(crate) parent_plant_eui: Eui,
    pub(crate) moisture: f32,
    pub(crate) temp: f32,
    pub(crate) ts: NaiveDateTime,
}
