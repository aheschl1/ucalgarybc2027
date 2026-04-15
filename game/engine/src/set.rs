use std::collections::HashMap;

use crate::{client::Client, entity::ClientEntity, game::GameState};


struct Set {
    clients: Vec<Client>,
    entities: Vec<ClientEntity>,
    entity_ownership: HashMap<u32, usize>,
    state: GameState
}

impl Set {
    fn new(clients: Vec<Client>) -> Self {
        Self {
            clients,
            entities: Vec::new(),
            entity_ownership: HashMap::new(),
            state: GameState::default(),
        }
    }

    async fn step(&mut self) {
        for entity in self.entities.iter() {
            let client = &mut self.clients[self.entity_ownership[&entity.id()]];
            client.step_entity(entity);
        }
    }
}

mod tests {
    use crate::{client::Client, set::Set};


    #[test]
    fn test_set() {
        // let client = Client::new();
        // let mut set = Set::new(vec![client]);
        // set.entities.push(&client.entities[0]);
    }
}