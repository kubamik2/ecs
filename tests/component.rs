mod common;
use ecs::*;
use common::*;

#[test]
fn get() {
    let mut world = World::default();
    let mut entities = vec![];
    for i in 0..100 {
        let entity = world.spawn((
            ComponentA::new(i),
            ComponentB(format!("{i}")),
        ));
        entities.push(entity);
    }
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentA>(entities[i]).expect("get_component ComponentA not found");
        assert!(component.validate(i), "ComponentA validation failed");
    });
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentB>(entities[i]).expect("get_component ComponentB not found");
        assert!(component.validate(i), "ComponentB validation failed");
    });
}

#[test]
fn set() {
    let mut world = World::default();
    let mut entities = vec![];
    for i in 0..100 {
        let entity = world.spawn((
            ComponentA::new(i),
            ComponentB(format!("{i}")),
        ));
        entities.push(entity);
    }
    (0..100).for_each(|i| {
        world.set_component(entities[i], ComponentA::new(i+1));
        world.set_component(entities[i], ComponentB(format!("{}", i+1)));
    });
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentA>(entities[i]).unwrap();
        assert!(component.validate(i+1), "ComponentA validation failed");
    });
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentB>(entities[i]).unwrap();
        assert!(component.validate(i+1), "ComponentB validation failed");
    });
}

#[test]
fn component_hooks() {
    struct Spawner;

    impl Component for Spawner {
        fn on_add(&mut self, commands: &mut Commands) {
            commands.spawn(Position);
        }

        fn on_remove(&mut self, commands: &mut Commands) {
            commands.spawn(Rotation);
        }
    }

    impl Resource for Spawner {
        fn on_add(&mut self, commands: &mut Commands) {
            commands.spawn(Position);
        }

        fn on_remove(&mut self, commands: &mut Commands) {
            commands.spawn(Rotation);
        }
    }

    #[derive(Component)]
    struct Position;
    #[derive(Component)]
    struct Rotation;

    let mut world = World::default();

    let entity1 = world.spawn(Spawner);
    let entity2 = world.spawn(Spawner);
    assert!(world.query::<&Position>().iter().count() == 2);
    world.despawn(entity1);
    world.despawn(entity2);
    assert!(world.query::<&Rotation>().iter().count() == 2);

    world.insert_resource(Spawner);
    world.insert_resource(Spawner);
    assert!(world.query::<&Position>().iter().count() == 4);
    world.remove_resource::<Spawner>();
    assert!(world.query::<&Rotation>().iter().count() == 4);
}
