mod common;
use ecs::*;

#[test]
fn disallow_race_condition() {
    use std::sync::atomic::{Ordering, AtomicBool};
    #[derive(Resource)]
    struct A {
        is_locked: AtomicBool
    }

    let mut world = World::new(16).unwrap();
    let mut schedule = Schedule::default();

    world.insert_resource(A { is_locked: AtomicBool::new(false) });
    for _ in 0..16 {
        schedule.add_system(|a: ResMut<A>| {
            a.is_locked.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).expect("Race condition occurred");
            std::thread::sleep(std::time::Duration::from_millis(50));
            a.is_locked.store(false, Ordering::Relaxed);   
        });
    }
    schedule.run(&mut world);
}

#[test]
#[should_panic]
fn disallow_missing_resource() {
    #[derive(Resource)] struct A;
    let mut world = World::new(1).unwrap();
    let mut schedule = Schedule::default();
    schedule.add_system(|_: Res<A>| {});
    schedule.run(&mut world);
}
