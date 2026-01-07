## Responsibilities of the Dispatcher Storage Layer


- Accept and persist incoming SensorReading events

- Accept and persist incoming DeviceStatus events

- Store events locally in a durable way

- Track whether an event has been uploaded or not

- Allow reading all pending (not yet uploaded) events

- Allow marking events as successfully uploaded

- Be usable with different backends (memory, SQLite)
