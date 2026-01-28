local g = import 'github.com/grafana/grafonnet/gen/grafonnet-latest/main.libsonnet';
local common = import '../lib/common.libsonnet';
local ch = import '../lib/clickhouse.libsonnet';

// Current value stats
local currentAirTemp = common.statPanel(
  'Current Air Temp',
  |||
    SELECT avg(metric_value) as value
    FROM (
      SELECT device_id, argMax(metric_value, timestamp) as metric_value
      FROM sensor_readings
      WHERE metric_type = 2
      GROUP BY device_id
    )
  |||,
  'celsius'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('thresholds')
+ g.panel.stat.standardOptions.thresholds.withSteps([
  { color: 'blue', value: null },
  { color: 'green', value: 15 },
  { color: 'yellow', value: 25 },
  { color: 'orange', value: 30 },
  { color: 'red', value: 35 },
]);

local currentSoilTemp = common.statPanel(
  'Current Soil Temp',
  |||
    SELECT avg(metric_value) as value
    FROM (
      SELECT device_id, argMax(metric_value, timestamp) as metric_value
      FROM sensor_readings
      WHERE metric_type = 1
      GROUP BY device_id
    )
  |||,
  'celsius'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('thresholds')
+ g.panel.stat.standardOptions.thresholds.withSteps([
  { color: 'blue', value: null },
  { color: 'green', value: 10 },
  { color: 'yellow', value: 20 },
  { color: 'orange', value: 25 },
  { color: 'red', value: 30 },
]);

local currentHumidity = common.statPanel(
  'Current Humidity',
  |||
    SELECT avg(metric_value) as value
    FROM (
      SELECT device_id, argMax(metric_value, timestamp) as metric_value
      FROM sensor_readings
      WHERE metric_type = 3
      GROUP BY device_id
    )
  |||,
  'percent'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('thresholds')
+ g.panel.stat.standardOptions.thresholds.withSteps([
  { color: 'red', value: null },
  { color: 'orange', value: 30 },
  { color: 'green', value: 40 },
  { color: 'green', value: 70 },
  { color: 'orange', value: 85 },
  { color: 'red', value: 95 },
]);

local currentSoilMoisture = common.statPanel(
  'Current Soil Moisture',
  |||
    SELECT avg(metric_value) as value
    FROM (
      SELECT device_id, argMax(metric_value, timestamp) as metric_value
      FROM sensor_readings
      WHERE metric_type = 0
      GROUP BY device_id
    )
  |||,
  'percent'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('thresholds')
+ g.panel.stat.standardOptions.thresholds.withSteps([
  { color: 'red', value: null },
  { color: 'orange', value: 20 },
  { color: 'green', value: 40 },
  { color: 'green', value: 70 },
  { color: 'orange', value: 85 },
]);

local currentRainfall = common.statPanel(
  'Total Rainfall (24h)',
  |||
    SELECT sum(metric_value) as value
    FROM sensor_readings
    WHERE metric_type = 4
      AND timestamp >= now() - INTERVAL 24 HOUR
  |||,
  'mm'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('thresholds')
+ g.panel.stat.standardOptions.thresholds.withSteps([
  { color: 'green', value: null },
  { color: 'blue', value: 5 },
  { color: 'blue', value: 20 },
]);

local readingsCount = common.statPanel(
  'Readings (Period)',
  |||
    SELECT count() as value
    FROM sensor_readings
    WHERE %s
  ||| % ch.timeFilter('timestamp')
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

// Time series panels
local airTempGraph = common.timeseriesPanel(
  'Air Temperature',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      device_id,
      avg(metric_value) as temperature
    FROM sensor_readings
    WHERE metric_type = 2
      AND %s
    GROUP BY time, device_id
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'celsius'
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.fieldConfig.defaults.custom.withFillOpacity(10);

local soilTempGraph = common.timeseriesPanel(
  'Soil Temperature',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      device_id,
      avg(metric_value) as temperature
    FROM sensor_readings
    WHERE metric_type = 1
      AND %s
    GROUP BY time, device_id
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'celsius'
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.fieldConfig.defaults.custom.withFillOpacity(10);

local humidityGraph = common.timeseriesPanel(
  'Humidity',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      device_id,
      avg(metric_value) as humidity
    FROM sensor_readings
    WHERE metric_type = 3
      AND %s
    GROUP BY time, device_id
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'percent'
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.standardOptions.withMin(0)
+ g.panel.timeSeries.standardOptions.withMax(100)
+ g.panel.timeSeries.fieldConfig.defaults.custom.withFillOpacity(20);

local soilMoistureGraph = common.timeseriesPanel(
  'Soil Moisture',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      device_id,
      avg(metric_value) as moisture
    FROM sensor_readings
    WHERE metric_type = 0
      AND %s
    GROUP BY time, device_id
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'percent'
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.standardOptions.withMin(0)
+ g.panel.timeSeries.standardOptions.withMax(100)
+ g.panel.timeSeries.fieldConfig.defaults.custom.withFillOpacity(20);

local rainfallGraph = common.timeseriesPanel(
  'Rainfall',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      sum(metric_value) as rainfall
    FROM sensor_readings
    WHERE metric_type = 4
      AND %s
    GROUP BY time
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'mm'
)
+ g.panel.timeSeries.gridPos.withW(24)
+ g.panel.timeSeries.gridPos.withH(6)
+ g.panel.timeSeries.fieldConfig.defaults.custom.withDrawStyle('bars')
+ g.panel.timeSeries.fieldConfig.defaults.custom.withFillOpacity(80);

common.dashboard('Sensor Readings', 'ersha-sensor-readings', ['sensors'])
+ g.dashboard.withVariables([
  common.datasourceRef,
  common.deviceVariable,
])
+ g.dashboard.withPanels([
  // Row 1: Current values
  currentAirTemp + g.panel.stat.gridPos.withX(0) + g.panel.stat.gridPos.withY(0),
  currentSoilTemp + g.panel.stat.gridPos.withX(4) + g.panel.stat.gridPos.withY(0),
  currentHumidity + g.panel.stat.gridPos.withX(8) + g.panel.stat.gridPos.withY(0),
  currentSoilMoisture + g.panel.stat.gridPos.withX(12) + g.panel.stat.gridPos.withY(0),
  currentRainfall + g.panel.stat.gridPos.withX(16) + g.panel.stat.gridPos.withY(0),
  readingsCount + g.panel.stat.gridPos.withX(20) + g.panel.stat.gridPos.withY(0),

  // Row 2: Temperature graphs
  airTempGraph + g.panel.timeSeries.gridPos.withX(0) + g.panel.timeSeries.gridPos.withY(4),
  soilTempGraph + g.panel.timeSeries.gridPos.withX(12) + g.panel.timeSeries.gridPos.withY(4),

  // Row 3: Moisture/Humidity graphs
  humidityGraph + g.panel.timeSeries.gridPos.withX(0) + g.panel.timeSeries.gridPos.withY(12),
  soilMoistureGraph + g.panel.timeSeries.gridPos.withX(12) + g.panel.timeSeries.gridPos.withY(12),

  // Row 4: Rainfall
  rainfallGraph + g.panel.timeSeries.gridPos.withX(0) + g.panel.timeSeries.gridPos.withY(20),
])
