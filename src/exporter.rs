use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use prometheus_client_derive_encode::EncodeLabelSet;
use std::sync::Arc;
use tapo::Plug;
use tapo::responses::{
    CurrentPowerResult, DeviceInfoPowerStripResult, PowerStripPlugEnergyMonitoringResult,
};
use tapo::{Error, PowerStripEnergyMonitoringHandler};
use tokio::sync::RwLock;

#[async_trait]
pub trait TapoClient {
    async fn refresh_session(&mut self) -> Result<(), Error>;
    async fn device_info(&self) -> Result<DeviceInfoPowerStripResult, Error>;
    async fn child_devices(&self) -> Result<Vec<PowerStripPlugEnergyMonitoringResult>, Error>;
    async fn get_power_for_plug(&self, device_id: &str) -> Result<CurrentPowerResult, Error>;
}

#[derive(Debug)]
pub struct PowerStripClient {
    pub client: PowerStripEnergyMonitoringHandler,
}

#[async_trait]
impl TapoClient for PowerStripClient {
    async fn refresh_session(&mut self) -> Result<(), Error> {
        match self.client.refresh_session().await {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn device_info(&self) -> Result<DeviceInfoPowerStripResult, Error> {
        self.client.get_device_info().await
    }

    async fn child_devices(&self) -> Result<Vec<PowerStripPlugEnergyMonitoringResult>, Error> {
        self.client.get_child_device_list().await
    }

    async fn get_power_for_plug(&self, device_id: &str) -> Result<CurrentPowerResult, Error> {
        let plug = self
            .client
            .plug(Plug::ByDeviceId(device_id.to_string()))
            .await?;

        plug.get_current_power().await
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct PowerUse {
    pub power_strip_id: String,
    pub device_id: String,
    pub nickname: String,
    pub position: u8,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct DeviceInfo {
    pub power_strip_id: String,
    pub model: String,
    pub firmware_version: String,
}

struct AppState {
    pub registry: Registry,
    power_use: Family<PowerUse, Gauge>,
    device_info: Family<DeviceInfo, Gauge>,
    client: Box<dyn TapoClient + Send + Sync>,
}

impl AppState {
    pub async fn update_metrics(&mut self) -> Result<(), Error> {
        if let Err(e) = self.client.refresh_session().await {
            panic!("Failed to refresh session: {e}");
        }

        let device_info = self.client.device_info().await?;

        self.device_info
            .get_or_create(&DeviceInfo {
                power_strip_id: device_info.device_id.clone(),
                model: device_info.model,
                firmware_version: device_info.fw_ver,
            })
            .set(1);

        let child_device_list = self.client.child_devices().await?;

        for child in child_device_list.into_iter() {
            let current_power = self
                .client
                .get_power_for_plug(child.device_id.as_ref())
                .await?;

            self.power_use
                .get_or_create(&PowerUse {
                    power_strip_id: device_info.device_id.clone(),
                    device_id: child.device_id.clone(),
                    nickname: child.nickname,
                    position: child.position,
                })
                .set(current_power.current_power as i64);
        }

        Ok(())
    }
}

async fn metrics_handler(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut state = state.write().await;

    match state.update_metrics().await {
        Ok(_) => {
            let mut buffer = String::new();
            encode(&mut buffer, &state.registry).unwrap();

            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    "application/openmetrics-text; version=1.0.0; charset=utf-8",
                )
                .body(Body::from(buffer))
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(e.to_string()))
            .unwrap(),
    }
}

async fn health() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap()
}

pub fn app(power_strip: Box<dyn TapoClient + Send + Sync>) -> Router {
    let mut state = AppState {
        registry: Registry::default(),
        power_use: Family::default(),
        device_info: Family::default(),
        client: power_strip,
    };
    state.registry.register(
        "tapo_p304m_power_use_watts",
        "Current power use in watts",
        state.power_use.clone(),
    );
    state.registry.register(
        "tapo_p304m_device_info",
        "Device information",
        state.device_info.clone(),
    );
    let state = Arc::new(RwLock::new(state));

    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health))
        .with_state(state)
}

#[cfg(test)]
mod test {
    use super::TapoClient;
    use super::app;
    use async_trait::async_trait;

    use axum::body::Body;
    use axum::http::Request;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use tapo::Error;
    use tapo::responses::{
        AutoOffStatus, ChargingStatus, CurrentPowerResult, DefaultPlugState,
        DeviceInfoPowerStripResult, OvercurrentStatus, PowerProtectionStatus,
        PowerStripPlugEnergyMonitoringResult,
    };
    use tower::ServiceExt; // for `collect`

    struct TestClient {}

    #[async_trait]
    impl TapoClient for TestClient {
        async fn refresh_session(&mut self) -> Result<(), Error> {
            Ok(())
        }

        async fn device_info(&self) -> Result<DeviceInfoPowerStripResult, Error> {
            Ok(DeviceInfoPowerStripResult {
                avatar: "".to_string(),
                device_id: "123".to_string(),
                fw_id: "".to_string(),
                fw_ver: "".to_string(),
                has_set_location_info: false,
                hw_id: "".to_string(),
                hw_ver: "".to_string(),
                ip: "".to_string(),
                lang: "".to_string(),
                latitude: None,
                longitude: None,
                mac: "".to_string(),
                model: "catwalk".to_string(),
                oem_id: "".to_string(),
                region: None,
                rssi: 0,
                signal_level: 0,
                specs: "".to_string(),
                ssid: "".to_string(),
                time_diff: 0,
                r#type: "".to_string(),
            })
        }

        async fn child_devices(&self) -> Result<Vec<PowerStripPlugEnergyMonitoringResult>, Error> {
            let mut l = Vec::new();
            l.push(PowerStripPlugEnergyMonitoringResult {
                auto_off_remain_time: 0,
                auto_off_status: AutoOffStatus::On,
                avatar: "".to_string(),
                bind_count: 0,
                category: "".to_string(),
                default_states: DefaultPlugState::LastStates {},
                charging_status: ChargingStatus::Finished,
                device_id: "456".to_string(),
                device_on: false,
                fw_id: "".to_string(),
                fw_ver: "".to_string(),
                has_set_location_info: false,
                hw_id: "".to_string(),
                hw_ver: "".to_string(),
                is_usb: false,
                latitude: None,
                longitude: None,
                mac: "".to_string(),
                model: "".to_string(),
                nickname: "".to_string(),
                oem_id: "".to_string(),
                on_time: 0,
                original_device_id: "".to_string(),
                overcurrent_status: OvercurrentStatus::Lifted,
                overheat_status: None,
                position: 1,
                power_protection_status: PowerProtectionStatus::Normal,
                region: None,
                slot_number: 0,
                status_follow_edge: false,
                r#type: "".to_string(),
            });
            Ok(l)
        }

        async fn get_power_for_plug(&self, device_id: &str) -> Result<CurrentPowerResult, Error> {
            match device_id.as_ref() {
                "456" => Ok(CurrentPowerResult { current_power: 45 }),
                d => {
                    panic!("unexpected device_id {}", d);
                }
            }
        }
    }

    #[tokio::test]
    async fn get_metrics() {
        let client = Box::new(TestClient {});
        let app = app(client);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = str::from_utf8(body_bytes.as_ref()).unwrap();

        let expected = "# HELP tapo_p304m_power_use_watts Current power use in watts.\n\
        # TYPE tapo_p304m_power_use_watts gauge\n\
        tapo_p304m_power_use_watts{power_strip_id=\"123\",device_id=\"456\",nickname=\"\",position=\"1\"} 45\n\
        # HELP tapo_p304m_device_info Device information.\n\
        # TYPE tapo_p304m_device_info gauge\n\
        tapo_p304m_device_info{power_strip_id=\"123\",model=\"catwalk\",firmware_version=\"\"} 1\n\
        # EOF\n\
        ";
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn get_health() {
        let client = Box::new(TestClient {});
        let app = app(client);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
