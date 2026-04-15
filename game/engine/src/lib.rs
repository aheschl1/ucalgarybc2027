pub mod proto {
    include!("proto/v1/proto.v1.rs");
}


mod entity;
mod client;
mod set;
mod game;


impl From<(u32, u32)> for crate::proto::Vec2 {
    fn from(value: (u32, u32)) -> Self {
        Self { x: value.0 as f32, y: value.1 as f32 }
    }
}