// @generated automatically by Diesel CLI.

diesel::table! {
    gas_data (id) {
        id -> Integer,
        parent_plant_eui -> Binary,
        temp -> Float,
        pressure -> Float,
        humidity -> Float,
        gas -> Float,
        ts -> Timestamp,
    }
}

diesel::table! {
    light_data (id) {
        id -> Integer,
        parent_plant_eui -> Binary,
        lux -> Float,
        fs -> Float,
        ts -> Timestamp,
    }
}

diesel::table! {
    moisture_data (id) {
        id -> Integer,
        parent_plant_eui -> Binary,
        moisture -> Float,
        temp -> Float,
        ts -> Timestamp,
    }
}

diesel::table! {
    plants (eui) {
        eui -> Binary,
        name -> Text,
        species -> Text,
        addr -> Text,
        update_count -> Integer,
    }
}

diesel::joinable!(gas_data -> plants (parent_plant_eui));
diesel::joinable!(light_data -> plants (parent_plant_eui));
diesel::joinable!(moisture_data -> plants (parent_plant_eui));

diesel::allow_tables_to_appear_in_same_query!(gas_data, light_data, moisture_data, plants,);
