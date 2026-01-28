local g = import 'github.com/grafana/grafonnet/gen/grafonnet-latest/main.libsonnet';
local common = import '../lib/common.libsonnet';
local ch = import '../lib/clickhouse.libsonnet';

local batteryLevels = common.timeseriesPanel(
  'Battery Levels Over Time',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      device_id,
      avg(battery_percent) as battery
    FROM device_statuses
    WHERE %s
    GROUP BY time, device_id
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'percent'
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.standardOptions.withMin(0)
+ g.panel.timeSeries.standardOptions.withMax(100)
+ g.panel.timeSeries.standardOptions.thresholds.withSteps([
  { color: 'red', value: null },
  { color: 'orange', value: 20 },
  { color: 'yellow', value: 50 },
  { color: 'green', value: 80 },
]);

local signalStrength = common.timeseriesPanel(
  'Signal Strength (RSSI)',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      device_id,
      avg(signal_rssi) as rssi
    FROM device_statuses
    WHERE %s
    GROUP BY time, device_id
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'dBm'
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8)
+ g.panel.timeSeries.standardOptions.thresholds.withSteps([
  { color: 'red', value: null },
  { color: 'orange', value: -80 },
  { color: 'yellow', value: -70 },
  { color: 'green', value: -50 },
]);

local uptimePanel = common.timeseriesPanel(
  'Device Uptime',
  |||
    SELECT
      $__timeInterval(timestamp) AS time,
      device_id,
      max(uptime_seconds) / 3600 as uptime_hours
    FROM device_statuses
    WHERE %s
    GROUP BY time, device_id
    ORDER BY time
  ||| % ch.timeFilter('timestamp'),
  'h'
)
+ g.panel.timeSeries.gridPos.withW(12)
+ g.panel.timeSeries.gridPos.withH(8);

local errorDistribution = common.barGaugePanel(
  'Error Distribution',
  |||
    SELECT
      error_code,
      count() as count
    FROM device_status_errors
    GROUP BY error_code
    ORDER BY count DESC
    LIMIT 10
  |||
)
+ g.panel.barGauge.gridPos.withW(12)
+ g.panel.barGauge.gridPos.withH(8)
+ g.panel.barGauge.options.withDisplayMode('gradient')
+ g.panel.barGauge.options.withOrientation('horizontal');

local lowBatteryDevices = common.tablePanel(
  'Low Battery Devices',
  |||
    SELECT
      ds.device_id,
      d.manufacturer,
      ds.battery_percent,
      ds.signal_rssi,
      ds.timestamp as last_seen
    FROM (
      SELECT
        device_id,
        argMax(battery_percent, timestamp) as battery_percent,
        argMax(signal_rssi, timestamp) as signal_rssi,
        max(timestamp) as timestamp
      FROM device_statuses
      GROUP BY device_id
    ) ds
    LEFT JOIN (
      SELECT id, manufacturer FROM devices FINAL
    ) d ON ds.device_id = d.id
    WHERE ds.battery_percent < 30
    ORDER BY ds.battery_percent ASC
  |||
)
+ g.panel.table.gridPos.withW(24)
+ g.panel.table.gridPos.withH(8)
+ g.panel.table.standardOptions.withOverrides([
  {
    matcher: { id: 'byName', options: 'battery_percent' },
    properties: [
      { id: 'unit', value: 'percent' },
      { id: 'custom.cellOptions', value: { type: 'gauge', mode: 'gradient' } },
      { id: 'thresholds', value: {
        mode: 'absolute',
        steps: [
          { color: 'red', value: null },
          { color: 'orange', value: 10 },
          { color: 'yellow', value: 20 },
        ],
      } },
    ],
  },
  {
    matcher: { id: 'byName', options: 'signal_rssi' },
    properties: [
      { id: 'unit', value: 'dBm' },
    ],
  },
]);

local currentBatteryGauges = common.gaugePanel(
  'Current Battery Levels',
  |||
    SELECT
      device_id,
      argMax(battery_percent, timestamp) as battery
    FROM device_statuses
    GROUP BY device_id
    ORDER BY battery ASC
    LIMIT 10
  |||,
  'percent',
  0,
  100
)
+ g.panel.gauge.gridPos.withW(12)
+ g.panel.gauge.gridPos.withH(6)
+ g.panel.gauge.standardOptions.thresholds.withSteps([
  { color: 'red', value: null },
  { color: 'orange', value: 20 },
  { color: 'yellow', value: 50 },
  { color: 'green', value: 80 },
]);

local currentRssiGauges = common.gaugePanel(
  'Current Signal Strength',
  |||
    SELECT
      device_id,
      argMax(signal_rssi, timestamp) as rssi
    FROM device_statuses
    GROUP BY device_id
    ORDER BY rssi ASC
    LIMIT 10
  |||,
  'dBm',
  -100,
  0
)
+ g.panel.gauge.gridPos.withW(12)
+ g.panel.gauge.gridPos.withH(6)
+ g.panel.gauge.standardOptions.thresholds.withSteps([
  { color: 'red', value: null },
  { color: 'orange', value: -80 },
  { color: 'yellow', value: -70 },
  { color: 'green', value: -50 },
]);

common.dashboard('Device Health', 'ersha-device-health', ['health'])
+ g.dashboard.withVariables([
  common.datasourceRef,
  common.deviceVariable,
])
+ g.dashboard.withPanels([
  batteryLevels + g.panel.timeSeries.gridPos.withX(0) + g.panel.timeSeries.gridPos.withY(0),
  signalStrength + g.panel.timeSeries.gridPos.withX(12) + g.panel.timeSeries.gridPos.withY(0),
  uptimePanel + g.panel.timeSeries.gridPos.withX(0) + g.panel.timeSeries.gridPos.withY(8),
  errorDistribution + g.panel.barGauge.gridPos.withX(12) + g.panel.barGauge.gridPos.withY(8),
  currentBatteryGauges + g.panel.gauge.gridPos.withX(0) + g.panel.gauge.gridPos.withY(16),
  currentRssiGauges + g.panel.gauge.gridPos.withX(12) + g.panel.gauge.gridPos.withY(16),
  lowBatteryDevices + g.panel.table.gridPos.withX(0) + g.panel.table.gridPos.withY(22),
])
