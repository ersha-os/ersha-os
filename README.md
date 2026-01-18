# ersha-os

**Open-source Digital Public Infrastructure (DPI) for Smart Agriculture Monitoring**

An affordable, scalable, LoRaWAN-powered platform that helps governments, cooperatives, NGOs, and individual farmers monitor farms in real time, optimize irrigation, protect water infrastructure, and make better crop decisions.

*Made for smallholder farmers, built to scale for nations.*

---

### ✨ Key Features

- Real-time **sensor monitoring**
- Long-range **LoRaWAN** wireless connectivity (up to 10–15 km in rural areas)
- Low-power sensors
- Mobile + web dashboard with alerts (push, SMS, Telegram, WhatsApp)
- Scalable: single farm → regional → national deployment
- Fully open-source & hardware-agnostic

### 🏗️ Architecture

<center>
<img alt="LoRaWAN Architecture" src="https://github.com/user-attachments/assets/3224cf56-e945-4c2e-be62-c0ef82d0928c" />
</center>

- **End Devices** — Sensors (soil moisture, water level, weather) send LoRa messages
- **Gateways** — Forward messages to the Network Server
- **Network Server** — Manages the network (e.g. ChirpStack, The Things Stack)
- **Application Server** — Processes data, stores time-series, sends alerts, provides dashboard

### 🔗 Related Projects

- [8028 Farmer Hotline](https://ati.gov.et/8028-farmer-hotline/)

### 📄 License

MIT

### 🤝 Contributing

Contributions are welcome!
