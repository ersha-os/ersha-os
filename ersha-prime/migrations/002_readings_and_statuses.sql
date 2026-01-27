CREATE TABLE IF NOT EXISTS readings (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL,
    dispatcher_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    metric_type INTEGER NOT NULL,
    metric_value REAL NOT NULL,
    location INTEGER NOT NULL,
    confidence INTEGER NOT NULL,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_readings_device_id ON readings(device_id);
CREATE INDEX IF NOT EXISTS idx_readings_sensor_id ON readings(sensor_id);
CREATE INDEX IF NOT EXISTS idx_readings_dispatcher_id ON readings(dispatcher_id);
CREATE INDEX IF NOT EXISTS idx_readings_timestamp ON readings(timestamp);

CREATE TABLE IF NOT EXISTS device_statuses (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL,
    dispatcher_id TEXT NOT NULL,
    battery_percent INTEGER NOT NULL,
    uptime_seconds INTEGER NOT NULL,
    signal_rssi INTEGER NOT NULL,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_device_statuses_device_id ON device_statuses(device_id);
CREATE INDEX IF NOT EXISTS idx_device_statuses_timestamp ON device_statuses(timestamp);

CREATE TABLE IF NOT EXISTS device_status_errors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    status_id TEXT NOT NULL,
    error_code INTEGER NOT NULL,
    message TEXT,
    FOREIGN KEY(status_id) REFERENCES device_statuses(id)
);

CREATE INDEX IF NOT EXISTS idx_device_status_errors_status_id ON device_status_errors(status_id);

CREATE TABLE IF NOT EXISTS device_status_sensor_statuses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    status_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    state INTEGER NOT NULL,
    last_reading INTEGER,
    FOREIGN KEY(status_id) REFERENCES device_statuses(id)
);

CREATE INDEX IF NOT EXISTS idx_device_status_sensor_statuses_status_id ON device_status_sensor_statuses(status_id);
