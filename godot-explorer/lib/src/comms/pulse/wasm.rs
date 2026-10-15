use super::transport::{
    PulseDisconnect, PulseDriverChannels, PulseReliability, PulseStatus, PulseTransportConfig,
};
use crate::scene_runner::tokio_runtime::TokioRuntime;
use dcl_web_pulse::Event;

pub(super) fn spawn(config: PulseTransportConfig, channels: PulseDriverChannels) -> i32 {
    let PulseDriverChannels {
        mut outbound,
        inbound,
        status,
    } = channels;
    let _ = status.try_send(PulseStatus::Connecting);
    let host = if config.host.contains(':') && !config.host.starts_with('[') {
        format!("[{}]", config.host)
    } else {
        config.host
    };
    let id = dcl_web_pulse::open(&format!("https://{host}:{}/", config.port), move |event| {
        match event {
            Event::Connected => {
                let _ = status.try_send(PulseStatus::Connected);
            }
            Event::Message { bytes, reliable } => {
                if inbound.try_send(bytes).is_err() && reliable {
                    return false;
                }
            }
            Event::Disconnected(code) => {
                let _ =
                    status.try_send(PulseStatus::Disconnected(PulseDisconnect::from_code(code)));
            }
            Event::Failed => {
                let _ = status.try_send(PulseStatus::Failed(
                    "WebTransport connection failed".to_owned(),
                ));
            }
        }
        true
    });
    TokioRuntime::spawn(async move {
        while let Some(frame) = outbound.recv().await {
            let channel = match frame.reliability {
                PulseReliability::Reliable => 0,
                PulseReliability::UnreliableSequenced => 1,
                PulseReliability::UnreliableUnsequenced => 2,
            };
            if !dcl_web_pulse::send(id, channel, &frame.bytes) {
                break;
            }
        }
        dcl_web_pulse::close(id);
    });
    id
}
