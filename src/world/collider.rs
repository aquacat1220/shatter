use crate::math::*;
slotmap::new_key_type! { pub struct ColliderKey; }

#[derive(Debug)]
pub struct Collider {
    pub shape: Shape,
}
