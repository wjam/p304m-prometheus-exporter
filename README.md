# p304m-prometheus-exporter

A [Prometheus](https://prometheus.io/) exporter for the tp-link
[Tapo P304M Smart Wi-Fi Power Strip](https://www.tp-link.com/uk/home-networking/smart-plug/tapo-p304m/) or
[Tapo P110M Smart Plug](https://www.tp-link.com/uk/home-networking/smart-plug/tapo-p110m/).

| Metric name          | Description                                      |
|----------------------|--------------------------------------------------|
| tapo_power_use_watts | Current power use reported by each plug in watts |
| tapo_device_info     | Device information reported by the power strip   |

## TODO
- Only refresh session every _x_ minutes rather than on every call
  - https://users.rust-lang.org/t/schedule-a-blocking-task-every-x-minutes/115041/17
- Include energy usage as a metric?
