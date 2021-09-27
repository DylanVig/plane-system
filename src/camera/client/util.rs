use anyhow::Context;
use num_traits::FromPrimitive;
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use tokio::task::block_in_place;

use super::*;

pub(super) fn update(
    client: &mut CameraClient,
) -> anyhow::Result<HashMap<CameraPropertyCode, (ptp::PtpData, Option<ptp::PtpData>)>> {
    let CameraClient { interface, state } = client;
    
    trace!("ptp update");
    let properties =
        block_in_place(|| interface.update()).context("error while receiving camera state")?;

    let mut changes = HashMap::new();

    for property in properties {
        if let Some(property_code) =
            <CameraPropertyCode as FromPrimitive>::from_u16(property.property_code)
        {
            let new = property.current.clone();
            let old = state.insert(property_code, property).map(|d| d.current);

            if let Some(old) = old {
                if new != old {
                    changes.insert(property_code, (new, Some(old)));
                }
            } else {
                changes.insert(property_code, (new, None));
            }
        }
    }

    Ok(changes)
}

pub(super) fn set(
    interface: &mut CameraInterface,
    property: CameraPropertyCode,
    value: ptp::PtpData,
) -> anyhow::Result<()> {
    trace!("ptp set {:?} {:?}", property, value);
    block_in_place(|| interface.set(property, value))
        .context("error while setting camera state")?;

    Ok(())
}

pub fn control(
    interface: &mut CameraInterface,
    action: CameraControlCode,
    value: ptp::PtpData,
) -> anyhow::Result<()> {
    trace!("ptp execute {:?} {:?}", action, value);
    block_in_place(|| interface.execute(action, value))
        .context("error while setting camera state")?;

    Ok(())
}

pub(super) async fn ensure(
    client: &mut CameraClient,
    property: CameraPropertyCode,
    value: ptp::PtpData,
) -> anyhow::Result<()> {
    loop {
        let actual = client.state.get(&property);

        if let Some(actual) = actual {
            if actual.current == value {
                break;
            }
        }

        set(&mut client.interface, property, value.clone())?;

        update(client)?;
    }

    Ok(())
}

pub(super) async fn ensure_mode(
    client: &mut CameraClient,
    mode: CameraOperatingMode,
) -> anyhow::Result<()> {
    ensure(
        client,
        CameraPropertyCode::OperatingMode,
        ptp::PtpData::UINT8(mode as u8),
    )
    .await
}

pub(super) async fn watch(
    client: &RwLock<CameraClient>,
    property: CameraPropertyCode,
) -> anyhow::Result<(ptp::PtpData, Option<ptp::PtpData>)> {
    loop {
        let mut changes = {
            trace!("watch: locking client for write");
            let mut client = client.write().await;
            trace!("watch: locked client for write");
            update(&mut *client)?
        };

        trace!("watch: unlocked client for write");

        if let Some(change) = changes.remove(&property) {
            break Ok(change);
        }

        tokio::task::yield_now().await;
    }
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

pub(super) fn download(
    interface: &mut CameraInterface,
    object_handle: ptp::ObjectHandle,
) -> anyhow::Result<(ptp::PtpObjectInfo, Vec<u8>)> {
    trace!("download: getting object info");

    let object_info = block_in_place(|| interface.object_info(object_handle, Some(TIMEOUT)))
        .context("error getting object info for download")?;

    trace!("download: getting object data");

    let object_data = block_in_place(|| interface.object_data(object_handle, Some(TIMEOUT)))
        .context("error getting object data for download")?;

    trace!("download: got object data");

    Ok((object_info, object_data))
}
