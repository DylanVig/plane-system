use serde::Serialize;

#[derive(Debug, Clone)]
pub enum CameraEvent {
  
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive, Serialize)]
pub enum CameraExposureMode {
    ManualExposure = 0x0001,
    ProgramAuto,
    AperturePriority,
    ShutterPriority,
    IntelligentAuto = 0x8000,
    SuperiorAuto,
    MovieProgramAuto = 0x8050,
    MovieAperturePriority,
    MovieShutterPriority,
    MovieManualExposure,
    MovieIntelligentAuto,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive, Serialize)]
pub enum CameraCompressionMode {
    Std = 0x02,
    Fine = 0x03,
    RawJpeg = 0x13,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive, Serialize)]
pub enum CameraSaveMode {
    HostDevice = 0x0001,
    MemoryCard1 = 0x0002,
}
