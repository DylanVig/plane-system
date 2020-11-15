use serde::Serialize;

#[derive(Debug, Clone)]
pub enum CameraEvent {
    Error(CameraErrorMode)
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive, Serialize, Eq, PartialEq)]
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
#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive, Serialize, Eq, PartialEq)]
pub enum CameraCompressionMode {
    Std = 0x02,
    Fine = 0x03,
    RawJpeg = 0x13,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive, Serialize, Eq, PartialEq)]
pub enum CameraSaveMode {
    HostDevice = 0x0001,
    MemoryCard1 = 0x0002,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive, Serialize, Eq, PartialEq)]
pub enum CameraErrorMode {
    /// Hardware failure, etc
    Fatal = 0x8000,

    /// Error of recording still imageand movie, etc
    RecordingFailed = 0x4000,

    /// Full of still image, movie, etc.
    RecordingFailedStorageFull = 0x2000,

    /// Full of memory card, etc.
    RecordingFailedMediaFull = 0x1000,

    /// Data error, access error of memory card, etc.
    Media = 0x0800,

    /// Unsupported memory card, etc.
    UnsupportedMedia = 0x0400,

    /// Error of unsupported imagesize, etc.
    IncompatibleMedia = 0x0200,

    /// Media none
    NoMedia = 0x0100,

    /// During the recovery of media
    MediaInRecovery = 0x0080,

    MediaRecoveryFailed = 0x0040,

    Temperature = 0x0020,

    Battery = 0x0010,

    Reserved = 0x0008,

    LensNotRecognized = 0x0004,

    CaptureOnCapturing = 0x0002,

    SettingFailure = 0x0001,
}
