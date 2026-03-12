use itertools::Itertools;

use crate::event::Event;
use crate::math::{Shape, Vec2};
use crate::world::collider::ColliderKey;
use crate::world::{BodyHandle, World};

#[derive(Debug, Default)]
pub struct Engine {}

#[derive(Debug)]
struct Contact {
    collider_1: ColliderKey,
    collider_2: ColliderKey,
    contact_position_1: Vec2,
    contact_position_2: Vec2,
    contact_normal: Vec2,
    penetration_depth: f32, // `(contact_position_2 - contact_position_1).dot(contact_normal) == penetration_depth`
}

impl Engine {
    pub fn tick(&mut self, world: &mut World, dt: f32) -> Vec<Event> {
        self.update_velocity(world);
        let contact_candidates = self.broadphase(world);
        let contacts = self.narrowphase(world, contact_candidates);
        let events: Vec<Event> = contacts
            .iter()
            .map(|contact| {
                let body_key_1 = world.colliders.get(contact.collider_1).unwrap().body_key;
                let body_key_2 = world.colliders.get(contact.collider_2).unwrap().body_key;
                let body_1 = BodyHandle {
                    world_id: world.world_id,
                    body_key: body_key_1,
                };
                let body_2 = BodyHandle {
                    world_id: world.world_id,
                    body_key: body_key_2,
                };
                Event::Contact { body_1, body_2 }
            })
            .collect();
        self.solve(world, contacts);
        self.update_position(world, dt);

        events
    }

    fn update_velocity(&mut self, world: &mut World) {
        world.bodies.values_mut().for_each(|body| {
            body.velocity += body.accumulated_impulse * body.inverse_mass;
            body.accumulated_impulse = Vec2::ZERO;
        });
    }

    fn broadphase(&mut self, world: &World) -> Vec<(ColliderKey, ColliderKey)> {
        let collider_keys = world.colliders.keys();
        let collider_pairs = collider_keys.tuple_combinations::<(_, _)>();
        collider_pairs.collect()
    }

    fn narrowphase(
        &mut self,
        world: &World,
        contact_candidates: Vec<(ColliderKey, ColliderKey)>,
    ) -> Vec<Contact> {
        let mut contacts: Vec<Contact> = vec![];
        for (collider_key_1, collider_key_2) in contact_candidates {
            let collider_1 = world.colliders.get(collider_key_1).unwrap();
            let collider_2 = world.colliders.get(collider_key_2).unwrap();
            let body_1 = world.bodies.get(collider_1.body_key).unwrap();
            let body_2 = world.bodies.get(collider_2.body_key).unwrap();
            match (collider_1.shape, collider_2.shape) {
                (Shape::Circle(circle_1), Shape::Circle(circle_2)) => {
                    let d = body_2.position - body_1.position;
                    let d_mag = d.length();
                    let n = if d_mag > f32::EPSILON {
                        d * (1.0 / d_mag)
                    } else {
                        Vec2::RIGHT
                    };
                    let r = circle_1.radius + circle_2.radius;
                    if d_mag <= r {
                        contacts.push(Contact {
                            collider_1: collider_key_1,
                            collider_2: collider_key_2,
                            contact_position_1: body_1.position + n * circle_1.radius,
                            contact_position_2: body_2.position + n * circle_2.radius,
                            contact_normal: n,
                            penetration_depth: r - d_mag,
                        });
                    }
                }
            }
        }
        contacts
    }

    fn solve(&mut self, world: &mut World, contacts: Vec<Contact>) {}

    fn update_position(&mut self, world: &mut World, dt: f32) {
        world.bodies.values_mut().for_each(|body| {
            body.position += body.velocity * dt;
        });
    }
}
