mod common;
use ecs::*;

use crate::common::panic_unit;

#[test]
fn integration() {
    struct System(SystemId);

    impl Component for System {
        fn on_remove(&mut self, _: &mut Commands) {
            self.0.mark_dead();
        }
    }

    #[derive(Component)]
    struct Manager;

    #[derive(Component)]
    struct Parent;

    #[derive(Component)]
    struct Child;

    let mut world = World::default();

    let mut schedule = Schedule::default();
    let system_id = schedule.add_system(panic_unit);
    let manager = world.spawn(Manager);
    let system = world.spawn(System(system_id));
    world.add_child(manager, system);

    world.despawn(manager);
    schedule.run(&mut world);

    let a = world.spawn(Parent);
    let b = world.spawn(Child);
    let c = world.spawn(Child);
    let d = world.spawn(Child);
    world.add_child(a, b);
    world.add_child(a, c);
    world.add_child(a, d);

    assert_eq!(world.query_filtered::<Children, With<Parent>>().iter().next().unwrap().iter().count(), 3);
    world.despawn(a);
    assert_eq!(world.query::<Entity>().iter().count(), 0);

    schedule.add_system(|mut commands: Commands, query: Query<Entity, With<Parent>>| {
        for entity in query.iter() {
            let child = commands.spawn(Child);
            commands.add_child(entity, child);
        }
    });

    world.spawn(Parent);
    world.spawn(Parent);
    world.spawn(Parent);
    schedule.run(&mut world);

    for children in world.query_filtered::<Children, With<Parent>>().iter() {
        assert!(children.len() == 1);
    }

    let mut schedule = Schedule::default();
    schedule.add_system(|mut commands: Commands, query: Query<Entity, With<Parent>>| {
        for entity in query.iter() {
            commands.remove_children(entity);
        }
    });

    schedule.run(&mut world);

    for children in world.query_filtered::<Children, With<Parent>>().iter() {
        assert!(children.is_empty());
    }

    let mut world = World::default();
    let mut schedule = Schedule::default();
    let sys_a = schedule.add_system(panic_unit);
    let sys_b = schedule.add_system(panic_unit);

    let parent = world.spawn(Parent);
    let child_a = world.spawn(Child);
    let child_b = world.spawn(Child);
    let child_c = world.spawn(System(sys_a));
    let child_d = world.spawn(System(sys_b));
    world.add_child(parent, child_a);
    world.add_child(parent, child_b);
    world.add_child(child_a, child_c);
    world.add_child(child_b, child_d);
    world.despawn(parent);
    schedule.run(&mut world);
}
