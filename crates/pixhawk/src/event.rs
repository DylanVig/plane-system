use ps_types::{Euler, Point3D, Velocity3D};

#[derive(Debug, Clone)]
pub enum PixhawkEvent {
    Gps {
        position: Point3D,
        /// Velocity in meters per second (X, Y, Z) / (East, North, Up)
        velocity: Velocity3D,
    },
    Orientation {
        attitude: Euler,
    },
}
