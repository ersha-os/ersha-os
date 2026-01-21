CREATE TABLE IF NOT EXISTS sensor_readings (
    id TEXT PRIMARY KEY,
    reading_json TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'uploaded')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    uploaded_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS device_statuses (
    id TEXT PRIMARY KEY,
    status_json TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'uploaded')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    uploaded_at TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_sensor_readings_state 
ON sensor_readings(state);

CREATE INDEX IF NOT EXISTS idx_device_statuses_state 
ON device_statuses(state);

CREATE INDEX IF NOT EXISTS idx_sensor_readings_created_at 
ON sensor_readings(created_at);

CREATE INDEX IF NOT EXISTS idx_device_statuses_created_at 
ON device_statuses(created_at);

CREATE INDEX IF NOT EXISTS idx_sensor_readings_uploaded_at 
ON sensor_readings(uploaded_at);

CREATE INDEX IF NOT EXISTS idx_device_statuses_uploaded_at 
ON device_statuses(uploaded_at);
