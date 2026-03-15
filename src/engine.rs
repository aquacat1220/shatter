use itertools::Itertools;

use crate::event::Event;
use crate::math::{Shape, Vec2};
use crate::world::body::BodyKey;
use crate::world::collider::ColliderKey;
use crate::world::{BodyHandle, World};

#[derive(Debug)]
pub struct Engine {
    pgs_steps: u32,
    bounce_threshold: f32,
    bounce_coeff: f32,
    baumgarte_coeff: f32,
}

impl Default for Engine {
    fn default() -> Self {
        Engine {
            pgs_steps: 20,
            bounce_threshold: 0.000005,
            bounce_coeff: 0.95,
            baumgarte_coeff: 0.25,
        }
    }
}

#[derive(Debug)]
struct Contact {
    collider_1: ColliderKey,
    collider_2: ColliderKey,
    body_1: BodyKey,
    body_2: BodyKey,
    contact_position_1: Vec2,
    contact_position_2: Vec2,
    contact_normal: Vec2,
    penetration_depth: f32, // `(contact_position_1 - contact_position_2).dot(contact_normal) == penetration_depth`
}

impl Engine {
    pub fn tick(&mut self, world: &mut World, dt: f32) -> Vec<Event> {
        self.update_velocity(world);
        let contact_candidates = self.broadphase(world);
        let contacts = self.narrowphase(world, contact_candidates);
        let events: Vec<Event> = contacts
            .iter()
            .map(|contact| {
                let body_key_1 = world.colliders.get(contact.collider_1).unwrap().body_key; // Safety: We just generated these contacts during narrowphase.
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
        self.solve(world, contacts, dt);
        self.update_position(world, dt);

        events
    }

    fn update_velocity(&mut self, world: &mut World) {
        world.bodies.values_mut().for_each(|body| {
            body.velocity += body.accumulated_impulse * body.mass_inv;
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
            let collider_1 = world.colliders.get(collider_key_1).unwrap(); // Safety: The candidate keys come straight from broadphase, which fetched the keys fresh from the world collider slotmap.
            let collider_2 = world.colliders.get(collider_key_2).unwrap();
            let body_1 = world.bodies.get(collider_1.body_key).unwrap(); // Safety: And the body keys come straight from the colliders, which are guaranteed to have valid body keys.
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
                            body_1: collider_1.body_key,
                            body_2: collider_2.body_key,
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

    fn solve(&mut self, world: &mut World, contacts: Vec<Contact>, dt: f32) {
        // We are solving for J M_inv J_T lambda + J v_t + b >= 0
        let mut accumulated_lambdas: Box<[f32]> = vec![0.0; contacts.len()].into_boxed_slice();
        let mut biases: Box<[f32]> = vec![0.0; contacts.len()].into_boxed_slice();
        for i in 0..=self.pgs_steps {
            for (m, contact) in contacts.iter().enumerate() {
                // Assume this contact is the m-th contact for the comments below.
                let [body_1, body_2] = world
                    .bodies
                    .get_disjoint_mut([contact.body_1, contact.body_2])
                    .unwrap(); // Safety: Narrowphase is guaranteed to generate contacts with valid keys, and we never have self collisions (yet, because bodies only have a single collider).

                let eff_mass = body_1.mass_inv + body_2.mass_inv; // The m-th diagonal from the J M_inv J_T matrix.
                if eff_mass <= f32::EPSILON {
                    continue;
                } // This shouldn't be possible; why would we add collisions between static colliders to the solver? But just to keep things from breaking...

                // Compute delta-lambda.
                let relative_velocity =
                    (body_2.velocity - body_1.velocity).dot(&contact.contact_normal); // The relative velocity along the contact normal. Positive means they are moving away from each other. The m-th row of J v_t.
                if i == 0 {
                    // Bias calculation should be done only on the first tick to ensure correctness.
                    // Calculate restitution (bounce) term.
                    if relative_velocity < -self.bounce_threshold {
                        biases[m] += self.bounce_coeff * relative_velocity;
                    }
                    // Calculate Baumgarte Stabilization (penetration fix) term.
                    if contact.penetration_depth > f32::EPSILON {
                        biases[m] -= self.baumgarte_coeff * contact.penetration_depth / dt;
                    }
                    // TODO: Baumgarte Stabilization, predicted collisions, split impulses.
                }
                let bias = biases[m];
                let delta_lambda = -(relative_velocity + bias) * (1.0 / eff_mass); // The m-th row of delta-lambda.

                // Clamp lambdas.
                let old_accumulated_lambda = accumulated_lambdas[m];
                let new_accumulated_lambda = f32::max(0.0, old_accumulated_lambda + delta_lambda);
                let delta_lambda_clamped = new_accumulated_lambda - old_accumulated_lambda;

                // Apply lambda.
                body_1.velocity -= // Positive lambda should push bodies away from each other. This means body 2 should go towards normal, and body 1 should do the opposite.
                    contact.contact_normal * (delta_lambda_clamped * body_1.mass_inv);
                body_2.velocity +=
                    contact.contact_normal * (delta_lambda_clamped * body_2.mass_inv);

                accumulated_lambdas[m] = new_accumulated_lambda;
            }
        }
    }

    fn update_position(&mut self, world: &mut World, dt: f32) {
        world.bodies.values_mut().for_each(|body| {
            body.position += body.velocity * dt;
        });
    }
}
