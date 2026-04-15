pub enum EntityType {
    Bot = 0
}

pub struct ClientEntity {
    id: u32,
    position: (u32, u32),
    entity_type: EntityType,

}

impl ClientEntity {
    pub fn new(id: u32, position: (u32, u32), entity_type: EntityType) -> Self {
        Self {
            id,
            position,
            entity_type,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }
}

impl From<EntityType> for crate::proto::EntityType {
    fn from(value: EntityType) -> Self {
        match value {
            EntityType::Bot => Self::Bot,
        }
    }
}

impl From<ClientEntity> for crate::proto::Entity {
    fn from(value: ClientEntity) -> Self {
        let etype: crate::proto::EntityType = value.entity_type.into();
        crate::proto::Entity { 
            id: value.id, 
            position: Some(value.position.into()), 
            entity_type: etype.into()
        }
    }
}