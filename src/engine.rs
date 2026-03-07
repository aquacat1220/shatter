use crate::world::World;

#[derive(Debug, Default)]
pub struct Engine {}

impl Engine {
    pub fn tick(&mut self, world: &mut World, dt: f32) {}
}
