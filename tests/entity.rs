mod common;
use ecs::*;

#[test]
fn integration() {
    #[derive(Component)] struct A;
    #[derive(Component)] struct B;
    #[derive(Component)] struct C;
    #[derive(Component)] struct D;
    #[derive(Component)] struct E;
    #[derive(Component)] struct F;
    #[derive(Component)] struct G;

    let mut world = World::default();

    let a = world.spawn(A);
    let b = world.spawn(B);
    let c = world.spawn(C);
    let d = world.spawn(D);
    let e = world.spawn(E);
    let f = world.spawn(F);
    let g = world.spawn(G);

    let old_b = b;
    let old_d = d;
    let old_f = f;

    assert!(world.query_filtered::<Entity, With<A>>().iter().next().unwrap() == a);
    assert!(world.query_filtered::<Entity, With<B>>().iter().next().unwrap() == b);
    assert!(world.query_filtered::<Entity, With<C>>().iter().next().unwrap() == c);
    assert!(world.query_filtered::<Entity, With<D>>().iter().next().unwrap() == d);
    assert!(world.query_filtered::<Entity, With<E>>().iter().next().unwrap() == e);
    assert!(world.query_filtered::<Entity, With<F>>().iter().next().unwrap() == f);
    assert!(world.query_filtered::<Entity, With<G>>().iter().next().unwrap() == g);

    assert!(world.is_alive(a) && world.is_alive(b) && world.is_alive(c) && world.is_alive(d) && world.is_alive(e) && world.is_alive(f) && world.is_alive(g));

    world.despawn(b);
    world.despawn(d);
    world.despawn(f);

    assert!(world.is_alive(a) && !world.is_alive(b) && world.is_alive(c) && !world.is_alive(d) && world.is_alive(e) && !world.is_alive(f) && world.is_alive(g));

    assert!(world.query_filtered::<Entity, With<A>>().iter().next().unwrap() == a);
    assert!(world.query_filtered::<Entity, With<B>>().iter().next().is_none());
    assert!(world.query_filtered::<Entity, With<C>>().iter().next().unwrap() == c);
    assert!(world.query_filtered::<Entity, With<D>>().iter().next().is_none());
    assert!(world.query_filtered::<Entity, With<E>>().iter().next().unwrap() == e);
    assert!(world.query_filtered::<Entity, With<F>>().iter().next().is_none());
    assert!(world.query_filtered::<Entity, With<G>>().iter().next().unwrap() == g);

    let f = world.spawn(B);
    let d = world.spawn(D);
    let b = world.spawn(F);


    assert!(world.query_filtered::<Entity, With<A>>().iter().next().unwrap() == a);
    assert!(world.query_filtered::<Entity, With<B>>().iter().next().unwrap() == f);
    assert!(world.query_filtered::<Entity, With<C>>().iter().next().unwrap() == c);
    assert!(world.query_filtered::<Entity, With<D>>().iter().next().unwrap() == d);
    assert!(world.query_filtered::<Entity, With<E>>().iter().next().unwrap() == e);
    assert!(world.query_filtered::<Entity, With<F>>().iter().next().unwrap() == b);
    assert!(world.query_filtered::<Entity, With<G>>().iter().next().unwrap() == g);

    assert!(world.is_alive(a) && world.is_alive(b) && world.is_alive(c) && world.is_alive(d) && world.is_alive(e) && world.is_alive(f) && world.is_alive(g));

    assert!(!world.is_alive(old_b) && !world.is_alive(old_d) && !world.is_alive(old_f));

    world.despawn(a);
    world.despawn(b);
    world.despawn(c);
    world.despawn(d);
    world.despawn(e);
    world.despawn(f);
    world.despawn(g);

    assert!(!world.is_alive(a) && !world.is_alive(b) && !world.is_alive(c) && !world.is_alive(d) && !world.is_alive(e) && !world.is_alive(f) && !world.is_alive(g));

    world.spawn(A);

    assert!(!world.is_alive(a));
    assert!(!world.is_alive(b));
    assert!(!world.is_alive(c));
    assert!(!world.is_alive(d));
    assert!(!world.is_alive(e));
    assert!(!world.is_alive(f));
    assert!(!world.is_alive(g));
    assert!(!world.is_alive(old_b));
    assert!(!world.is_alive(old_f));
    assert!(!world.is_alive(old_d));
}
