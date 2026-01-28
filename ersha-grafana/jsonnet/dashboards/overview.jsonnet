local g = import 'github.com/grafana/grafonnet/gen/grafonnet-latest/main.libsonnet';
local common = import '../lib/common.libsonnet';
local ch = import '../lib/clickhouse.libsonnet';

local totalDevices = common.statPanel(
  'Total Devices',
  'SELECT count() FROM devices FINAL'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local activeDevices = common.statPanel(
  'Active Devices',
  "SELECT count() FROM devices FINAL WHERE state = 'Active'"
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('fixed')
+ g.panel.stat.standardOptions.color.withFixedColor('green');

local totalDispatchers = common.statPanel(
  'Dispatchers',
  'SELECT count() FROM dispatchers FINAL'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local totalReadings = common.statPanel(
  'Total Readings',
  |||
    SELECT count()
    FROM sensor_readings
    WHERE %s
  ||| % ch.timeFilter('timestamp')
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local errorCount = common.statPanel(
  'Errors',
  |||
    SELECT count()
    FROM device_status_errors
  |||
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('fixed')
+ g.panel.stat.standardOptions.color.withFixedColor('red');

local avgBattery = common.gaugePanel(
  'Avg Battery',
  |||
    SELECT avg(battery_percent)
    FROM (
      SELECT device_id, argMax(battery_percent, timestamp) as battery_percent
      FROM device_statuses
      GROUP BY device_id
    )
  |||,
  'percent',
  0,
  100
)
+ g.panel.gauge.gridPos.withW(4)
+ g.panel.gauge.gridPos.withH(4)
+ g.panel.gauge.standardOptions.thresholds.withSteps([
  { color: 'red', value: null },
  { color: 'orange', value: 20 },
  { color: 'yellow', value: 50 },
  { color: 'green', value: 80 },
]);

local readingsOverTime = common.timeseriesPanel(
  'Sensor Readings Over Time',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      count() as readings
    FROM sensor_readings
    WHERE %s
    GROUP BY time
    ORDER BY time
  ||| % ch.timeFilter('timestamp')
)
+ g.panel.timeSeries.gridPos.withW(24)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.fieldConfig.defaults.custom.withFillOpacity(20);

local readingsByType = common.timeseriesPanel(
  'Readings by Metric Type',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      CASE metric_type
        WHEN 0 THEN 'Soil Moisture'
        WHEN 1 THEN 'Soil Temperature'
        WHEN 2 THEN 'Air Temperature'
        WHEN 3 THEN 'Humidity'
        WHEN 4 THEN 'Rainfall'
        ELSE 'Unknown'
      END as metric,
      count() as readings
    FROM sensor_readings
    WHERE %s
    GROUP BY time, metric_type, metric
    ORDER BY time
  ||| % ch.timeFilter('timestamp')
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.fieldConfig.defaults.custom.withFillOpacity(20);

local devicesByManufacturer = common.barGaugePanel(
  'Devices by Manufacturer',
  |||
    SELECT
      manufacturer,
      count() as count
    FROM devices FINAL
    GROUP BY manufacturer
    ORDER BY count DESC
  |||
)
+ g.panel.barGauge.gridPos.withW(12)
+ g.panel.barGauge.gridPos.withH(8)
+ g.panel.barGauge.options.withDisplayMode('gradient')
+ g.panel.barGauge.options.withOrientation('horizontal');

common.dashboard('Ersha OS Overview', 'ersha-overview', ['overview'])
+ g.dashboard.withVariables([
  common.datasourceRef,
])
+ g.dashboard.withPanels(
  g.util.grid.makeGrid([
    totalDevices,
    activeDevices,
    totalDispatchers,
    totalReadings,
    errorCount,
    avgBattery,
  ], panelWidth=4, panelHeight=4)
  + [
    readingsOverTime + g.panel.timeSeries.gridPos.withY(4),
    readingsByType + g.panel.timeSeries.gridPos.withY(12),
    devicesByManufacturer + g.panel.barGauge.gridPos.withX(12) + g.panel.barGauge.gridPos.withY(12),
  ]
)
