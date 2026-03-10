use super::collider::ColliderKey;
use crate::math::*;

slotmap::new_key_type! { pub struct BodyKey; }

#[derive(Debug)]
pub struct Body {
    pub position: Vec2,
    pub velocity: Vec2,
    pub accumulated_impulse: Vec2,
    pub inverse_mass: f32,
    pub collider_key: ColliderKey,
}
