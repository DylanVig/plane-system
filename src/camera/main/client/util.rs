use tokio::sync::broadcast;

use super::*;

pub(super) async fn ensure(
    interface: &CameraInterfaceRequestBuffer,
    property: CameraPropertyCode,
    value: ptp::PtpData,
) -> anyhow::Result<()> {
    loop {
        let value = value.clone();
        let (actual, caution) = interface
            .enter(|i| async move {
                (
                    i.get_value(property).await,
                    i.get_value(CameraPropertyCode::Caution).await,
                )
            })
            .await;

        if let Some(caution) = caution {
            if let ptp::PtpData::UINT16(0x0001) = caution {
                warn!("camera caution flag is Setting Failed; aborting setting change");
                break;
            }
        }

        if let Some(actual) = actual {
            if &actual == &value {
                break;
            }
        }

        interface
            .enter(|i| async move {
                i.set_value(property, value.clone()).await?;
                i.update().await?;

                Ok::<_, anyhow::Error>(())
            })
            .await?;

        tokio::task::yield_now().await;
    }

    Ok(())
}

pub(super) async fn ensure_mode(
    interface: &CameraInterfaceRequestBuffer,
    mode: OperatingMode,
) -> anyhow::Result<()> {
    ensure(
        interface,
        CameraPropertyCode::OperatingMode,
        ptp::PtpData::UINT8(mode as u8),
    )
    .await
}

pub(super) async fn watch(
    interface: &CameraInterfaceRequestBuffer,
    property: CameraPropertyCode,
) -> anyhow::Result<(ptp::PtpData, Option<ptp::PtpData>)> {
    let initial = interface
        .enter(|i| async move { i.get_value(property).await })
        .await;

    let changed = loop {
        let actual = interface
            .enter(|i| async move {
                i.update().await?;

                let actual = i.get_value(property).await;

                Ok::<_, anyhow::Error>(actual)
            })
            .await?;

        if let Some(actual) = actual {
            if let Some(initial) = &initial {
                if &actual != initial {
                    break actual;
                }
            } else {
                break actual;
            }
        }

        tokio::task::yield_now().await;
    };

    Ok((changed, initial))
}

pub(super) async fn wait(
    ptp_rx: &mut broadcast::Receiver<ptp::PtpEvent>,
    event_code: ptp::EventCode,
) -> anyhow::Result<ptp::PtpEvent> {
    loop {
        let event = ptp_rx.recv().await?;

        trace!("wait: recv {:?}", event);

        if event.code == event_code {
            return Ok(event);
        }
    }
}
