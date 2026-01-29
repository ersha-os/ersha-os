# ersha-core

Shared type registry used to communicate between [`ersha-prime`](../ersha-prime) and [`ersha-dispatch`](../ersha-dispatch). All types are serializable via serde.

## Types

- ID newtypes — `DeviceId`, `ReadingId`, `StatusId`, `DispatcherId`, `BatchId`, `SensorId`, `AlertId`
- Value types — `H3Cell`, `Percentage`, `BoxStr`, `BoxList<T>`
- Domain models — `Device`, `Sensor`, `SensorReading`, `DeviceStatus`, `DeviceError`, `Dispatcher`
- Enums — `DeviceKind`, `DeviceState`, `DispatcherState`, `SensorKind`, `SensorState`, `SensorMetric`, `MetricUnit`, `DeviceErrorCode`, `AlertSeverity`, `AlertType`, `DisconnectionReason`
- RPC pairs — `HelloRequest`/`HelloResponse`, `BatchUploadRequest`/`BatchUploadResponse`, `AlertRequest`/`AlertResponse`, `DispatcherStatusRequest`/`DispatcherStatusResponse`, `DeviceDisconnectionRequest`/`DeviceDisconnectionResponse`
