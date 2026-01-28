# ersha-os

**Open-source Digital Public Infrastructure (DPI) for Smart Agriculture Monitoring**

An affordable, scalable, LoRaWAN-powered platform that helps governments, cooperatives, NGOs, and individual farmers monitor farms in real time, optimize irrigation, protect water infrastructure, and make better crop decisions.

*Made for smallholder farmers, built to scale for nations.*

---

## Key Features

- Real-time **sensor monitoring**
- Long-range **LoRaWAN** wireless connectivity (up to 10–15 km in rural areas)
- Low-power sensors
- Mobile + web dashboard with alerts (push, SMS, Telegram, WhatsApp)
- Scalable: single farm → regional → national deployment
- Fully open-source & hardware-agnostic

## Architecture

<center>
<img alt="LoRaWAN Architecture" src="https://github.com/user-attachments/assets/3224cf56-e945-4c2e-be62-c0ef82d0928c" />
</center>

## Workspace Overview

```
ersha
├── ersha-rpc        # Lightweight RPC & framing layer
├── ersha-dispatch   # Ingestion service for edge connections
├── ersha-prime      # Core backend & device registry
├── ersha-core       # Shared domain types and logic
├── ersha-dashboard  # Web dashboard (UI + e2e tests)
├── ersha-edge       # Edge-side SDK for embedded devices (no_std friendly)
├── examples         # Examples of use case of the ersha layers
```

---

## Crates

### [`ersha-rpc`](ersha-rpc)

Transport-agnostic RPC layer.

* Framing
* Message definitions
* Client/server helpers

---

### [`ersha-dispatch`](ersha-dispatch)

Ingress service for edge devices.

* Accepts connections (e.g. TCP)
* Validates and routes incoming data
* Pluggable storage backends (memory, SQLite)

---

### [`ersha-prime`](ersha-prime)

Primary backend service.

* Device registry
* Readings and status tracking
* SQLite and in-memory implementations
* Database migrations included

---

### [`ersha-core`](ersha-core)

Shared core types and domain logic used across crates.

---

### [`ersha-dashboard`](ersha-dashboard)

Web dashboard for monitoring devices and readings.

* Rust backend
* Web UI assets
* End-to-end tests via Playwright

---

### [`ersha-edge`](ersha-edge)

Edge client library used on embedded devices.

* Sensor registration
* Data encoding
* Transport abstraction
* Designed for constrained environments

See [`ersha-edge/README.md`](ersha-edge/README.md) for details.

---

## License

Licensed under the terms of the MIT License. See [`LICENSE`](LICENSE) for details.
