use crate::{component_manager::ComponentManager, query::{Query, QueryData}};

pub trait System {
    fn execute(&self, component_manager: &ComponentManager);
}

impl<D: QueryData> System for fn(Query<D>) {
    fn execute(&self, component_manager: &ComponentManager) {
        let query: Query<D> = Query::new(component_manager);
        (self)(query)
    }
}

#[derive(Default)]
pub struct Schedule {
    systems: Vec<Box<dyn System>>,
}

impl Schedule {
    pub fn add_system<D: QueryData + 'static>(&mut self, system: fn(Query<D>)) {
        self.systems.push(Box::new(system));
    }

    pub(crate) fn execute_all(&self, component_manger: &ComponentManager) {
        for system in &self.systems {
            system.execute(component_manger);
        }
    }
}

unsafe impl Send for Schedule {}
unsafe impl Sync for Schedule {}
