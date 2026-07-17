use std::collections::HashMap;

use uuid::Uuid;

use crate::database::provider::{DatabaseProvider, DatabaseResult};
use crate::models::connection::ConnectionConfig;

pub struct ConnectionPool {
    connections: HashMap<Uuid, Box<dyn DatabaseProvider>>,
    pub active_id: Option<Uuid>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            active_id: None,
        }
    }

    pub fn register(&mut self, id: Uuid, provider: Box<dyn DatabaseProvider>) {
        self.connections.insert(id, provider);
    }

    pub fn unregister(&mut self, id: Uuid) {
        self.connections.remove(&id);
        if self.active_id == Some(id) {
            self.active_id = None;
        }
    }

    pub fn get(&self, id: Uuid) -> Option<&dyn DatabaseProvider> {
        if let Some(provider) = self.connections.get(&id) {
            Some(provider.as_ref())
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut dyn DatabaseProvider> {
        if let Some(provider) = self.connections.get_mut(&id) {
            Some(provider.as_mut())
        } else {
            None
        }
    }

    pub fn get_active(&self) -> Option<&dyn DatabaseProvider> {
        let id = self.active_id?;
        self.get(id)
    }

    pub fn get_active_mut(&mut self) -> Option<&mut dyn DatabaseProvider> {
        let id = self.active_id?;
        self.get_mut(id)
    }

    pub fn set_active(&mut self, id: Uuid) {
        if self.connections.contains_key(&id) {
            self.active_id = Some(id);
        }
    }

    pub fn connect(
        &mut self,
        id: Uuid,
        provider: Box<dyn DatabaseProvider>,
        _config: &ConnectionConfig,
    ) -> DatabaseResult<()> {
        self.register(id, provider);
        self.set_active(id);
        Ok(())
    }

    pub fn disconnect(&mut self, id: Uuid) {
        self.unregister(id);
    }

    pub fn list(&self) -> Vec<Uuid> {
        self.connections.keys().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}
