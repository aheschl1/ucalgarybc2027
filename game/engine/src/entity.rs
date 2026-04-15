use crate::client::Client;

pub enum EntityType {
    Bot
}

pub struct Entity {
    client: Client,
    id: u32,
    entity_type: EntityType,
}

impl Entity {
    pub fn new(client: Client, id: u32, entity_type: EntityType) -> Self {
        Self {
            client,
            id,
            entity_type
        }
    }

    pub fn step(&mut self) {
        self.client.step_entity(self.id);
    }
}