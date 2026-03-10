use crate::world::BodyHandle;

#[derive(Debug, Clone, Copy)]
pub enum Event {
    DummyEvent,
    Contact {
        body_1: BodyHandle,
        body_2: BodyHandle,
    },
}
