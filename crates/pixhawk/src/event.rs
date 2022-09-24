use ps_types::{Point3D, Attitude};

#[derive(Debug, Clone)]
pub enum PixhawkEvent {
    Gps {
        position: Point3D,
        /// Velocity in meters per second (X, Y, Z) / (East, North, Up)
        velocity: (f32, f32, f32),
    },
    Orientation {
        attitude: Attitude,
    },
}
