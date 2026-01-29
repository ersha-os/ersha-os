# ersha-grafana

Ready to use grafana instance to visual metrics from a [`ersha-server`](../ersha-prime) instance.

## Deployment

```
docker compose up -d
```

## Environment varables

```env
GF_INSTALL_PLUGINS=grafana-clickhouse-
GF_SECURITY_ADMIN_USER=admin
GF_SECURITY_ADMIN_PASSWORD=admin
GF_USERS_ALLOW_SIGN_UP=false
GF_DASHBOARDS_DEFAULT_HOME_DASHBOARD_PATH=/var/lib/grafana/dashboards/overview.json
```
