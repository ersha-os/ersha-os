local g = import 'github.com/grafana/grafonnet/gen/grafonnet-latest/main.libsonnet';

{
  // Metric type mappings
  metricTypes:: {
    SoilMoisture: 0,
    SoilTemp: 1,
    AirTemp: 2,
    Humidity: 3,
    Rainfall: 4,
  },

  metricTypeNames:: {
    '0': 'Soil Moisture',
    '1': 'Soil Temperature',
    '2': 'Air Temperature',
    '3': 'Humidity',
    '4': 'Rainfall',
  },

  // Dashboard template with common settings
  dashboard(title, uid, tags=[])::
    g.dashboard.new(title)
    + g.dashboard.withUid(uid)
    + g.dashboard.withTags(['ersha-os'] + tags)
    + g.dashboard.withTimezone('browser')
    + g.dashboard.withRefresh('30s')
    + g.dashboard.time.withFrom('now-6h')
    + g.dashboard.time.withTo('now'),

  // ClickHouse datasource reference
  datasource:: {
    type: 'grafana-clickhouse-datasource',
    uid: '${DS_CLICKHOUSE}',
  },

  datasourceRef:: g.dashboard.variable.datasource.new('DS_CLICKHOUSE', 'grafana-clickhouse-datasource')
    + g.dashboard.variable.datasource.generalOptions.withLabel('ClickHouse'),

  // Device variable
  deviceVariable::
    g.dashboard.variable.query.new('device_id', 'SELECT DISTINCT id FROM devices FINAL')
    + g.dashboard.variable.query.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
    + g.dashboard.variable.query.generalOptions.withLabel('Device')
    + g.dashboard.variable.query.selectionOptions.withIncludeAll(true, 'All')
    + g.dashboard.variable.query.selectionOptions.withMulti(true)
    + g.dashboard.variable.query.refresh.onLoad(),

  // Dispatcher variable
  dispatcherVariable::
    g.dashboard.variable.query.new('dispatcher_id', 'SELECT DISTINCT id FROM dispatchers FINAL')
    + g.dashboard.variable.query.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
    + g.dashboard.variable.query.generalOptions.withLabel('Dispatcher')
    + g.dashboard.variable.query.selectionOptions.withIncludeAll(true, 'All')
    + g.dashboard.variable.query.selectionOptions.withMulti(true)
    + g.dashboard.variable.query.refresh.onLoad(),

  // ClickHouse query target helper
  clickhouseTarget(query, refId='A', format=1):: {
    datasource: $.datasource,
    rawSql: query,
    refId: refId,
    format: format,
    queryType: 'sql',
  },

  // Panel helpers
  statPanel(title, query, unit='none')::
    g.panel.stat.new(title)
    + g.panel.stat.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
    + g.panel.stat.queryOptions.withTargets([
      $.clickhouseTarget(query, 'A', 1),
    ])
    + g.panel.stat.standardOptions.withUnit(unit)
    + g.panel.stat.options.withGraphMode('none')
    + g.panel.stat.options.withColorMode('value')
    + g.panel.stat.standardOptions.color.withMode('fixed')
    + g.panel.stat.standardOptions.color.withFixedColor('blue'),

  timeseriesPanel(title, query, unit='none', legendMode='list')::
    g.panel.timeSeries.new(title)
    + g.panel.timeSeries.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
    + g.panel.timeSeries.queryOptions.withTargets([
      $.clickhouseTarget(query, 'A', 2),
    ])
    + g.panel.timeSeries.standardOptions.withUnit(unit)
    + g.panel.timeSeries.options.legend.withDisplayMode(legendMode),

  gaugePanel(title, query, unit='percent', min=0, max=100)::
    g.panel.gauge.new(title)
    + g.panel.gauge.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
    + g.panel.gauge.queryOptions.withTargets([
      $.clickhouseTarget(query, 'A', 1),
    ])
    + g.panel.gauge.standardOptions.withUnit(unit)
    + g.panel.gauge.standardOptions.withMin(min)
    + g.panel.gauge.standardOptions.withMax(max),

  tablePanel(title, query)::
    g.panel.table.new(title)
    + g.panel.table.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
    + g.panel.table.queryOptions.withTargets([
      $.clickhouseTarget(query, 'A', 2),
    ]),

  barGaugePanel(title, query, unit='none')::
    g.panel.barGauge.new(title)
    + g.panel.barGauge.queryOptions.withDatasource('grafana-clickhouse-datasource', '${DS_CLICKHOUSE}')
    + g.panel.barGauge.queryOptions.withTargets([
      $.clickhouseTarget(query, 'A', 2),
    ])
    + g.panel.barGauge.standardOptions.withUnit(unit),

  // Row helper
  row(title)::
    g.panel.row.new(title),
}
