mod common;
use ecs::*;

use crate::common::panic_unit;

#[test]
fn remove() {
    let mut world = World::default();
    let mut schedule = Schedule::default();
    let id = schedule.add_system(panic_unit);
    let id2 = id.clone();
    id.mark_dead();
    assert!(!id.is_alive() && !id2.is_alive());
    schedule.run(&mut world);
    assert!(schedule.is_empty());
}

#[test]
#[should_panic]
fn disallow_double_mut() {
    #[derive(Resource)] struct A;
    let mut world = World::new(1).unwrap();
    let mut schedule = Schedule::default();
    world.insert_resource(A);
    schedule.add_system(|_a: ResMut<A>, _b: ResMut<A>| {

    });
    schedule.run(&mut world);
}

#[test]
#[should_panic]
fn disallow_mut_immut() {
    #[derive(Resource)] struct A;
    let mut world = World::new(1).unwrap();
    let mut schedule = Schedule::default();
    world.insert_resource(A);
    schedule.add_system(|_a: ResMut<A>, _b: Res<A>| {

    });
    schedule.run(&mut world);
}

