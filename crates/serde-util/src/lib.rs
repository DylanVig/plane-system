use serde::ser::SerializeStruct;

// by default, chrono will format with 10 or so fractional digits but python's
// builtin iso datetime parser only supports 6 digits, so this makes it a pain
// for postprocessing
pub const ISO_8601_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.6f%:z";

pub fn serialize_time<S>(
    this: &chrono::DateTime<chrono::Local>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::ser::Serializer,
{
    serializer.collect_str(&this.format(ISO_8601_FORMAT).to_string())
}

pub fn serialize_point<S>(this: &geo::Point<f32>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::ser::Serializer,
{
    let mut serializer = serializer.serialize_struct("Point", 2)?;
    serializer.serialize_field("lat", &this.y())?;
    serializer.serialize_field("lon", &this.x())?;
    serializer.end()
}

pub fn parse_hex_u32(src: &str) -> Result<u32, ParseIntError> {
    u32::from_str_radix(src, 16)
}
