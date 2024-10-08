CREATE TABLE plants (
  eui BINARY PRIMARY KEY NOT NULL,
  name VARCHAR NOT NULL,
  species VARCHAR NOT NULL,
  addr VARCHAR NOT NULL,
  update_count INTEGER NOT NULL
);

CREATE TABLE moisture_data (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  parent_plant_eui BINARY NOT NULL REFERENCES plants(eui),
  moisture FLOAT NOT NULL,
  temp FLOAT NOT NULL,
  ts DATETIME NOT NULL
);

CREATE TABLE light_data (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  parent_plant_eui BINARY NOT NULL REFERENCES plants(eui),
  lux FLOAT NOT NULL,
  fs FLOAT NOT NULL,
  ts DATETIME NOT NULL
);

CREATE TABLE gas_data (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  parent_plant_eui BINARY NOT NULL REFERENCES plants(eui),
  temp FLOAT NOT NULL,
  pressure FLOAT NOT NULL,
  humidity FLOAT NOT NULL,
  gas FLOAT NOT NULL,
  ts DATETIME NOT NULL
);