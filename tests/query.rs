mod common;
use ecs::*;

#[test]
fn filter() {
    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;
    #[derive(Component)]
    struct C;
    #[derive(Component)]
    struct D;
    #[derive(Component)]
    struct E;
    let mut world = World::default();
    let a = world.spawn((A, B, C, D));
    let b = world.spawn(A);
    let _ = world.spawn(B);
    let _ = world.spawn(C);
    let _ = world.spawn(D);

    assert!(world.query_filtered::<(), With<(A, B, C, D)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<(A, B)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<(C, D)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<(B, C)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<A>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<B>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<C>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<D>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<(A, A, B, B)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<E>>().iter().count() == 0);
    assert!(world.query_filtered::<(), With<(A, E)>>().iter().count() == 0);

    assert!(world.query_filtered::<&A, With<A>>().get(a).is_some());
    assert!(world.query_filtered::<&A, Without<A>>().iter().count() == 0);
    assert!(world.query_filtered::<&A, Without<B>>().iter().count() == 1);
    assert!(world.query_filtered::<&A, Without<B>>().get(b).is_some());
    assert!(world.query_filtered::<(&A, &B, &C, &D), Without<A>>().iter().count() == 0);
}

#[test]
fn disallow_race_condition() {
    use std::sync::atomic::{Ordering, AtomicBool};
    #[derive(Component)]
    struct A {
        is_locked: AtomicBool
    }

    let mut world = World::new(16).unwrap();
    let mut schedule = Schedule::default();
    world.spawn(A { is_locked: AtomicBool::new(false) });

    for _ in 0..16 {
        schedule.add_system(|a: Query<&mut A>| {
            let a = a.iter().next().expect("no entity");
            a.is_locked.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).expect("Race condition occurred");
            std::thread::sleep(std::time::Duration::from_millis(50));
            a.is_locked.store(false, Ordering::Relaxed);   
        });
    }
    schedule.run(&mut world);
}
