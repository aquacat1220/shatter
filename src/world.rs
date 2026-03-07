pub(crate) mod body;
pub(crate) mod collider;

use crate::math::*;

#[derive(Debug)]
pub struct World {
    id: u32,
}

impl World {
    pub fn new(id: u32) -> Self {
        Self { id }
    }

    pub fn body_handles(&self) -> impl Iterator<Item = BodyHandle> {
        std::iter::once(BodyHandle { world_id: self.id })
    }

    pub fn body(&self, handle: BodyHandle) -> Result<BodyView<'_>, ()> {
        if self.id != handle.world_id {
            return Err(());
        }
        Ok(BodyView {
            world: self,
            handle,
        })
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new(213123) // TODO: Write proper unique id generation.
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyHandle {
    world_id: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct BodyView<'a> {
    world: &'a World,
    handle: BodyHandle,
}

impl BodyView<'_> {
    pub fn position(&self) -> Vec2 {
        Default::default() // TODO
    }

    pub fn velocity(&self) -> Vec2 {
        Default::default() // TODO
    }

    pub fn shape(&self) -> Shape {
        Shape::Circle(Circle::new(1.0).unwrap())
    }
}
