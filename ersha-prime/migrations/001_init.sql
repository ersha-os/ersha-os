CREATE TABLE IF NOT EXISTS dispatchers (
    id TEXT PRIMARY KEY NOT NULL,
    state INTEGER NOT NULL,
    location INTEGER NOT NULL,
    provisioned_at DATETIME NOT NULL
);

CREATE TABLE devices (
    id TEXT PRIMARY KEY,
    kind INTEGER,
    state INTEGER,
    location INTEGER,
    manufacturer TEXT,
    provisioned_at INTEGER,
    sensor_count INTEGER DEFAULT 0
);

CREATE TABLE sensors (
   id TEXT PRIMARY KEY,
   kind INTEGER,
   metric_type INTEGER,
   metric_value REAL,
   device_id TEXT,
   FOREIGN KEY(device_id) REFERENCES devices(id)
);

CREATE TRIGGER IF NOT EXISTS trg_increment_sensor_count
AFTER INSERT ON sensors
BEGIN
    UPDATE devices 
    SET sensor_count = sensor_count + 1 
    WHERE id = NEW.device_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_decrement_sensor_count
AFTER DELETE ON sensors
BEGIN
    UPDATE devices 
    SET sensor_count = sensor_count - 1 
    WHERE id = OLD.device_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_update_sensor_count
AFTER UPDATE OF device_id ON sensors
BEGIN
    UPDATE devices SET sensor_count = sensor_count - 1 WHERE id = OLD.device_id;
    UPDATE devices SET sensor_count = sensor_count + 1 WHERE id = NEW.device_id;
END;
