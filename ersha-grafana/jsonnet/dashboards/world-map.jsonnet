local g = import 'github.com/grafana/grafonnet/gen/grafonnet-latest/main.libsonnet';
local common = import '../lib/common.libsonnet';

// Geomap panel showing dispatcher locations
local dispatcherMap =
  g.panel.geomap.new('Dispatcher Locations')
  + g.panel.geomap.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
  + g.panel.geomap.queryOptions.withTargets([
    common.clickhouseTarget(|||
      SELECT
        d.id as name,
        h3ToGeo(toUInt64(d.location)).1 as latitude,
        h3ToGeo(toUInt64(d.location)).2 as longitude,
        d.location as h3_index,
        h3GetResolution(toUInt64(d.location)) as h3_resolution,
        CASE d.state
          WHEN 0 THEN 'Inactive'
          WHEN 1 THEN 'Active'
          ELSE 'Unknown'
        END as state,
        count(DISTINCT dev.id) as device_count
      FROM dispatchers d FINAL
      LEFT JOIN devices dev FINAL ON 1=1
      GROUP BY d.id, d.location, d.state
    |||, 'A', 2),
  ])
  + g.panel.geomap.options.withView({
    id: 'coords',
    lat: 9,
    lon: 39,
    zoom: 5,
  })
  + g.panel.geomap.options.withBasemap({
    type: 'default',
    name: 'World',
  })
  + g.panel.geomap.options.withLayers([
    {
      type: 'markers',
      name: 'Dispatchers',
      config: {
        showLegend: true,
        style: {
          size: {
            fixed: 10,
            min: 5,
            max: 20,
            field: 'device_count',
          },
          color: {
            fixed: 'blue',
          },
          symbol: {
            mode: 'fixed',
            fixed: 'img/icons/marker/circle.svg',
          },
          opacity: 0.8,
        },
      },
      location: {
        mode: 'coords',
        latitude: 'latitude',
        longitude: 'longitude',
      },
      tooltip: true,
    },
  ])
  + g.panel.geomap.options.withTooltip({
    mode: 'details',
  })
  + g.panel.geomap.gridPos.withW(12)
  + g.panel.geomap.gridPos.withH(16);

// Geomap panel showing device locations
local deviceMap =
  g.panel.geomap.new('Device Locations')
  + g.panel.geomap.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
  + g.panel.geomap.queryOptions.withTargets([
    common.clickhouseTarget(|||
      SELECT
        dev.id as name,
        h3ToGeo(toUInt64(dev.location)).1 as latitude,
        h3ToGeo(toUInt64(dev.location)).2 as longitude,
        dev.sensor_count,
        COALESCE(dev.manufacturer, 'Unknown') as manufacturer,
        CASE dev.state
          WHEN 0 THEN 'Inactive'
          WHEN 1 THEN 'Active'
          ELSE 'Unknown'
        END as state
      FROM devices dev FINAL
    |||, 'A', 2),
  ])
  + g.panel.geomap.options.withView({
    id: 'coords',
    lat: 9,
    lon: 39,
    zoom: 5,
  })
  + g.panel.geomap.options.withBasemap({
    type: 'default',
    name: 'World',
  })
  + g.panel.geomap.options.withLayers([
    {
      type: 'markers',
      name: 'Devices',
      config: {
        showLegend: true,
        style: {
          size: {
            fixed: 6,
            min: 4,
            max: 12,
            field: 'sensor_count',
          },
          color: {
            fixed: 'green',
          },
          symbol: {
            mode: 'fixed',
            fixed: 'img/icons/marker/circle.svg',
          },
          opacity: 0.7,
        },
      },
      location: {
        mode: 'coords',
        latitude: 'latitude',
        longitude: 'longitude',
      },
      tooltip: true,
    },
  ])
  + g.panel.geomap.options.withTooltip({
    mode: 'details',
  })
  + g.panel.geomap.gridPos.withW(12)
  + g.panel.geomap.gridPos.withH(16);

// Geomap panel showing sensor reading density as heatmap
local deviceHeatmap =
  g.panel.geomap.new('Sensor Reading Density')
  + g.panel.geomap.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
  + g.panel.geomap.queryOptions.withTargets([
    common.clickhouseTarget(|||
      SELECT
        h3ToGeo(toUInt64(sr.location)).1 as latitude,
        h3ToGeo(toUInt64(sr.location)).2 as longitude,
        count() as reading_count
      FROM sensor_readings sr
      GROUP BY sr.location
    |||, 'A', 2),
  ])
  + g.panel.geomap.options.withView({
    id: 'coords',
    lat: 9,
    lon: 39,
    zoom: 5,
  })
  + g.panel.geomap.options.withBasemap({
    type: 'default',
    name: 'World',
  })
  + g.panel.geomap.options.withLayers([
    {
      type: 'markers',
      name: 'Reading Density',
      config: {
        showLegend: true,
        style: {
          size: {
            fixed: 8,
            min: 4,
            max: 20,
            field: 'reading_count',
          },
          color: {
            fixed: 'orange',
          },
          symbol: {
            mode: 'fixed',
            fixed: 'img/icons/marker/circle.svg',
          },
          opacity: 0.7,
        },
      },
      location: {
        mode: 'coords',
        latitude: 'latitude',
        longitude: 'longitude',
      },
      tooltip: true,
    },
  ])
  + g.panel.geomap.options.withTooltip({
    mode: 'details',
  })
  + g.panel.geomap.gridPos.withW(12)
  + g.panel.geomap.gridPos.withH(12);

// H3 Cell Details table
local h3CellDetails = common.tablePanel(
  'H3 Cell Details',
  |||
    SELECT
      d.id as dispatcher_id,
      hex(d.location) as h3_hex,
      h3GetResolution(toUInt64(d.location)) as resolution,
      h3ToGeo(toUInt64(d.location)).1 as latitude,
      h3ToGeo(toUInt64(d.location)).2 as longitude,
      h3CellAreaM2(toUInt64(d.location)) / 1000000 as cell_area_km2,
      CASE d.state
        WHEN 0 THEN 'Inactive'
        WHEN 1 THEN 'Active'
        ELSE 'Unknown'
      END as state
    FROM dispatchers d FINAL
    ORDER BY d.id
  |||
)
+ g.panel.table.gridPos.withW(12)
+ g.panel.table.gridPos.withH(12);

// Device Details table
local deviceDetails = common.tablePanel(
  'Device Details',
  |||
    SELECT
      dev.id as device_id,
      hex(dev.location) as h3_hex,
      h3ToGeo(toUInt64(dev.location)).1 as latitude,
      h3ToGeo(toUInt64(dev.location)).2 as longitude,
      dev.sensor_count,
      COALESCE(dev.manufacturer, 'Unknown') as manufacturer,
      CASE dev.state
        WHEN 0 THEN 'Inactive'
        WHEN 1 THEN 'Active'
        ELSE 'Unknown'
      END as state
    FROM devices dev FINAL
    ORDER BY dev.id
    LIMIT 100
  |||
)
+ g.panel.table.gridPos.withW(12)
+ g.panel.table.gridPos.withH(12);

// Stats row
local totalDispatchers = common.statPanel(
  'Total Dispatchers',
  'SELECT count() FROM dispatchers FINAL'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local activeDispatchers = common.statPanel(
  'Active Dispatchers',
  'SELECT count() FROM dispatchers FINAL WHERE state = 1'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('fixed')
+ g.panel.stat.standardOptions.color.withFixedColor('green');

local uniqueH3Cells = common.statPanel(
  'Unique H3 Cells',
  'SELECT count(DISTINCT location) FROM dispatchers FINAL'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local avgH3Resolution = common.statPanel(
  'Avg H3 Resolution',
  'SELECT avg(h3GetResolution(toUInt64(location))) FROM dispatchers FINAL'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local totalCoverage = common.statPanel(
  'Total Coverage',
  'SELECT sum(h3CellAreaM2(toUInt64(location))) / 1000000 FROM (SELECT DISTINCT location FROM dispatchers FINAL)',
  'areaKM2'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local totalDevices = common.statPanel(
  'Total Devices',
  'SELECT count() FROM devices FINAL'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

local activeDevices = common.statPanel(
  'Active Devices',
  'SELECT count() FROM devices FINAL WHERE state = 1'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4)
+ g.panel.stat.standardOptions.color.withMode('fixed')
+ g.panel.stat.standardOptions.color.withFixedColor('semi-dark-blue');

local uniqueDeviceCells = common.statPanel(
  'Unique Device Cells',
  'SELECT count(DISTINCT location) FROM devices FINAL'
)
+ g.panel.stat.gridPos.withW(4)
+ g.panel.stat.gridPos.withH(4);

common.dashboard('World Map', 'ersha-world-map', ['map', 'h3'])
+ g.dashboard.withVariables([
  common.datasourceRef,
])
+ g.dashboard.withPanels([
  // Row 1: Stats
  totalDispatchers + g.panel.stat.gridPos.withX(0) + g.panel.stat.gridPos.withY(0),
  activeDispatchers + g.panel.stat.gridPos.withX(4) + g.panel.stat.gridPos.withY(0),
  totalDevices + g.panel.stat.gridPos.withX(8) + g.panel.stat.gridPos.withY(0),
  activeDevices + g.panel.stat.gridPos.withX(12) + g.panel.stat.gridPos.withY(0),
  uniqueDeviceCells + g.panel.stat.gridPos.withX(16) + g.panel.stat.gridPos.withY(0),
  totalCoverage + g.panel.stat.gridPos.withX(20) + g.panel.stat.gridPos.withY(0),

  // Row 2: Location maps side by side
  dispatcherMap + g.panel.geomap.gridPos.withX(0) + g.panel.geomap.gridPos.withY(4),
  deviceMap + g.panel.geomap.gridPos.withX(12) + g.panel.geomap.gridPos.withY(4),

  // Row 3: Heatmap and details
  deviceHeatmap + g.panel.geomap.gridPos.withX(0) + g.panel.geomap.gridPos.withY(20),
  deviceDetails + g.panel.table.gridPos.withX(12) + g.panel.table.gridPos.withY(20),

  // Row 4: H3 cell details
  h3CellDetails + g.panel.table.gridPos.withX(0) + g.panel.table.gridPos.withY(32),
])
