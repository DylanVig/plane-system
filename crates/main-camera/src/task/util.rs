use crate::interface::PropertyCode;
use anyhow::{bail, Context};
use log::debug;
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;

use super::InterfaceGuard;

/// Converts a raw PTP data object into something that implements
/// `FromPrimitive` (such as an enum).
pub(crate) fn from_ptp_primitive<T: FromPrimitive>(ptp: &ptp::Data) -> Option<T> {
    match *ptp {
        ptp::Data::INT8(x) => T::from_i8(x),
        ptp::Data::UINT8(x) => T::from_u8(x),
        ptp::Data::INT16(x) => T::from_i16(x),
        ptp::Data::UINT16(x) => T::from_u16(x),
        ptp::Data::INT32(x) => T::from_i32(x),
        ptp::Data::UINT32(x) => T::from_u32(x),
        ptp::Data::INT64(x) => T::from_i64(x),
        ptp::Data::UINT64(x) => T::from_u64(x),
        _ => None,
    }
}

/// Gets the value of a camera property from a map returned by
/// [`CameraInterface::query`] and converts it to something that implements
/// `FromPrimitive` (such as an enum).
pub(super) fn convert_camera_value<T: FromPrimitive>(
    values: &HashMap<PropertyCode, ptp::PropInfo>,
    prop: PropertyCode,
) -> anyhow::Result<T> {
    from_ptp_primitive::<T>(
        values
            .get(&prop)
            .map(|p| &p.current)
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
pub(super) async fn ensure_camera_value(
    interface: &RwLock<InterfaceGuard>,
    prop: PropertyCode,
    value: ptp::Data,
) -> anyhow::Result<()> {
    debug!("ensuring {prop:?} is set to {value:?}");

    for _ in 0..10 {
        let props = interface.write().await.query()?;

        let actual_value = props.get(&prop).map(|p| &p.current);

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
