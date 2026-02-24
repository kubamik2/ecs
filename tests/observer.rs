mod common;
use ecs::*;

#[test]
fn simple() {
    #[derive(Resource)]
    struct Count(u32);

    struct Tick;

    let mut world = World::default();
    world.insert_resource(Count(0));
    for _ in 0..2 {
        world.add_observer(|_: Trigger<Tick>, mut count: ResMut<Count>| {
            count.0 += 1;
        });
    }

    for _ in 0..100 {
        world.trigger(Tick, None).unwrap();
    }

    assert!(world.resource::<Count>().0 == 200);
}
