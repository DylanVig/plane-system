use crate::interface::CameraInterface;
use crate::interface::PropertyCode;
use anyhow::{bail, Context};
use log::{debug, trace};
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;

/// Converts a raw PTP data object into something that implements
/// `FromPrimitive` (such as an enum).
pub(crate) fn from_ptp_primitive<T: FromPrimitive>(ptp: &ptp::PtpData) -> Option<T> {
    match *ptp {
        ptp::PtpData::INT8(x) => T::from_i8(x),
        ptp::PtpData::UINT8(x) => T::from_u8(x),
        ptp::PtpData::INT16(x) => T::from_i16(x),
        ptp::PtpData::UINT16(x) => T::from_u16(x),
        ptp::PtpData::INT32(x) => T::from_i32(x),
        ptp::PtpData::UINT32(x) => T::from_u32(x),
        ptp::PtpData::INT64(x) => T::from_i64(x),
        ptp::PtpData::UINT64(x) => T::from_u64(x),
        _ => None,
    }
}

/// Gets an up-to-date map from camera properties to their current values.
pub(crate) async fn get_camera_values(
    interface: &RwLock<CameraInterface>,
) -> anyhow::Result<HashMap<PropertyCode, ptp::PtpData>> {
    trace!("reading camera property values");

    Ok(interface
        .read()
        .await
        // get list of props and values from camera
        .update()
        .context("failed to get camera state")?
        // convert list of props into hashmap from prop code to value
        .into_iter()
        // extract property code as key and ignore properties w/ unknown codes
        .filter_map(|p| match PropertyCode::from_u16(p.property_code) {
            Some(code) => Some((code, p.current)),
            None => None,
        })
        .collect::<HashMap<_, _>>())
}

/// Gets the value of a camera property from a map returned by
/// [`get_camera_values`] and converts it to something that implements
/// `FromPrimitive` (such as an enum).
pub(crate) fn convert_camera_value<T: FromPrimitive>(
    values: &HashMap<PropertyCode, ptp::PtpData>,
    prop: PropertyCode,
) -> anyhow::Result<T> {
    from_ptp_primitive::<T>(
        values
            .get(&prop)
            .context(format!("value of property {:?} is unknown", prop))?,
    )
    .context(format!(
        "value of property {:?} is not a valid {}",
        prop,
        std::any::type_name::<T>()
    ))
}

/// Sets the value of a camera property, sleeps for a bit, and checks to make
/// sure that this value took effect. Will return immediately if the property's
/// current value is the same as `value`. Will fail after attempting to set the
/// value 10 times.
pub(crate) async fn ensure_camera_value(
    interface: &RwLock<CameraInterface>,
    prop: PropertyCode,
    value: ptp::PtpData,
) -> anyhow::Result<()> {
    debug!("ensuring {prop:?} is set to {value:?}");

    for _ in 0..10 {
        let props = get_camera_values(interface).await?;

        let actual_value = props.get(&prop);

        match actual_value {
            Some(actual_value) => {
                if &value == actual_value {
                    return Ok(());
                }

                interface.write().await.set(prop, value.clone())?;

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            None => bail!(
                "cannot ensure value of property {prop:?} because its current value is unknown"
            ),
        }
    }

    bail!("cannot ensure value of property {prop:?} because its value failed to change after 10 attempts")
}
