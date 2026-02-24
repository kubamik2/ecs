mod common;
use ecs::*;

#[test]
fn integration() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static X: AtomicU32 = AtomicU32::new(0);
    #[derive(Resource)]
    struct Count(u32);
    let mut world = World::default();
    world.insert_resource(Count(0));
    world.add_observer(|_: Trigger<Changed<Count>>| {
        X.fetch_add(1, Ordering::Relaxed);
    });
    let mut schedule = Schedule::default();
    schedule.add_system(|mut count: ResMut<Count>| {
        count.0 += 1;
    });

    let mut count = world.get_resource_mut::<Count>().unwrap();
    count.0 += 1;
    drop(count);

    schedule.run(&mut world);
    assert!(X.load(Ordering::Relaxed) == 2, "{}", X.load(Ordering::Relaxed));
}
