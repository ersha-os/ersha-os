{
  // Time filter macro for ClickHouse
  // Use $__fromTime and $__toTime for time range filtering
  timeFilter(column='timestamp')::
    '%s >= $__fromTime AND %s <= $__toTime' % [column, column],

  // Device filter helper (for use with multi-value variable)
  deviceFilter(column='device_id', variable='device_id')::
    "(%s IN (${%s:singlequote}) OR '${%s}' = 'All')" % [column, variable, variable],

  // Dispatcher filter helper
  dispatcherFilter(column='dispatcher_id', variable='dispatcher_id')::
    "(%s IN (${%s:singlequote}) OR '${%s}' = 'All')" % [column, variable, variable],

  // Common query patterns

  // Count query with optional time filter
  countQuery(table, timeColumn=null)::
    if timeColumn != null then
      'SELECT count() FROM %s WHERE %s' % [table, self.timeFilter(timeColumn)]
    else
      'SELECT count() FROM %s' % table,

  // Count query for ReplacingMergeTree tables (use FINAL)
  countQueryFinal(table)::
    'SELECT count() FROM %s FINAL' % table,

  // Average query
  avgQuery(table, column, timeColumn=null)::
    if timeColumn != null then
      'SELECT avg(%s) FROM %s WHERE %s' % [column, table, self.timeFilter(timeColumn)]
    else
      'SELECT avg(%s) FROM %s' % [column, table],

  // Time series query with interval
  timeSeriesQuery(select, table, timeColumn='timestamp', groupBy='$__interval')::
    |||
      SELECT
        $__timeInterval(%s) AS time,
        %s
      FROM %s
      WHERE %s
      GROUP BY time
      ORDER BY time
    ||| % [timeColumn, select, table, self.timeFilter(timeColumn)],

  // Time series with device grouping
  timeSeriesByDevice(select, table, timeColumn='timestamp')::
    |||
      SELECT
        $__timeInterval(%s) AS time,
        device_id,
        %s
      FROM %s
      WHERE %s
      GROUP BY time, device_id
      ORDER BY time
    ||| % [timeColumn, select, table, self.timeFilter(timeColumn)],
}
