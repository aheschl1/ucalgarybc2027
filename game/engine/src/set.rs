use crate::{client::Client, entity::Entity};


struct Set {
    clients: Vec<Client>,
    entities: Vec<Entity>,
}

impl Set {
    fn new(clients: Vec<Client>) -> Self {
        Self {
            clients,
            entities: Vec::new(),
        }
    }

    async fn step(&mut self) {
        for entity in self.entities.iter_mut() {
            entity.step();
        }
    }
}